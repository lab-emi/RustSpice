//! Adaptive transient: waveform breakpoints, LTE step control, `.ic` / `uic` start points, `tstart` and `tmax`.
//!
//! Every expected value is a closed form of the RC / RL circuit (not a RustSpice run), so these tests are not
//! circular. Tolerances are a few times the accuracy the default `reltol = 1e-3` promises.

use rustspice_core::netlist::AnalysisCmd;
use rustspice_solver::{Simulator, TranResult, TranSpec};

/// Runs the deck's `.tran` card with every field it carries (`tstart`, `tmax`, `uic`).
fn run(deck: &str) -> (Simulator, TranResult) {
    let mut sim = Simulator::from_spice(deck).unwrap();
    let spec = sim
        .netlist
        .analyses
        .iter()
        .find_map(|a| match *a {
            AnalysisCmd::Tran {
                tstep,
                tstop,
                tstart,
                tmax,
                uic,
            } => Some(TranSpec {
                tstep,
                tstop,
                tstart,
                tmax,
                uic,
            }),
            _ => None,
        })
        .expect("deck has a .tran card");
    let res = sim.run_transient_spec(&spec).unwrap();
    (sim, res)
}

fn node(sim: &Simulator, res: &TranResult, name: &str) -> Vec<f64> {
    let id = sim.netlist.find_node(name).unwrap();
    res.waveforms.iter().map(|x| x[id - 1]).collect()
}

/// RC low-pass response at `t` to a source that steps from 0 to `v` at `t0` (ideal edge).
fn rc_step(t: f64, t0: f64, v: f64, tau: f64) -> f64 {
    if t <= t0 {
        0.0
    } else {
        v * (1.0 - (-(t - t0) / tau).exp())
    }
}

#[test]
fn pulse_narrower_than_the_step_is_not_lost() {
    // 5 V for 100 ps (10 ps edges) into R = 10, C = 1 nF (tau = 10 ns), sampled every 10 ns. The injected charge
    // gives v(out) = 20.35 mV at the 110 ns row; a simulator that samples PULSE only at the rows sees nothing.
    let deck = "narrow pulse\nV1 in 0 PULSE(0 5 100n 10p 10p 100p 1u)\nR1 in out 10\nC1 out 0 1n\n.tran 10n 1u\n.end\n";
    let (sim, res) = run(deck);
    let v = node(&sim, &res, "out");
    let k = res
        .time_points
        .iter()
        .position(|&t| (t - 110e-9).abs() < 1e-15)
        .unwrap();
    // closed form: charge 5 V * 110 ps / 10 ohm (trapezoid area) ~ 55 pC on 1 nF then decay over 9.88 ns
    let tau: f64 = 10e-9;
    let area = 5.0 * (100e-12 + 10e-12); // V*s of the trapezoid
    let peak = area / tau; // small-pulse limit: v = area / tau, then exp decay from the pulse centre (~100.06 ns)
    let expect = peak * (-(110e-9 - 100.06e-9) / tau).exp();
    assert!(
        (v[k] - expect).abs() < 0.03 * expect,
        "v(out)(110 ns) = {:.4e}, closed form {:.4e}",
        v[k],
        expect
    );
}

#[test]
fn coarse_step_over_a_fast_edge_tracks_the_exponential() {
    // tau = 1 us with tstep = 3 us: the 1 ns edge at 6 us must be a breakpoint and the decay must be resolved
    // by internal steps; rows stay on the 3 us grid.
    let deck = "coarse\nV1 in 0 PULSE(0 1 6u 1n 1n 18u 36u)\nR1 in out 1k\nC1 out 0 1n\n.tran 3u 72u\n.end\n";
    let (sim, res) = run(deck);
    assert_eq!(res.time_points.len(), 25);
    for (k, &t) in res.time_points.iter().enumerate() {
        assert!((t - 3e-6 * k as f64).abs() <= 1e-18, "row {k} at {t:e}");
    }
    let v = node(&sim, &res, "out");
    let tau = 1e-6;
    for (k, &t) in res.time_points.iter().enumerate() {
        // rising edge at 6 us (+1 ns ramp), falling edge at 24.001 us
        let rise = rc_step(t, 6.0005e-6, 1.0, tau);
        let fall = rc_step(t, 24.0015e-6, 1.0, tau);
        let exact = if t < 42e-6 { rise - fall } else { f64::NAN };
        if exact.is_finite() {
            assert!(
                (v[k] - exact).abs() < 5e-3,
                "row {k} (t = {t:e}): {} vs {exact}",
                v[k]
            );
        }
    }
    assert!(
        res.accepted_steps > res.time_points.len(),
        "internal steps must refine the 3 us grid"
    );
}

#[test]
fn slope_kinks_do_not_ring_in_the_branch_current() {
    // tau = 1 ns RC driven by a 1 us ramp, tstep = 100 tau: the source current must be the smooth -C dv/dt
    // (-1 uA on the rising ramp), not the trapezoid's sign-alternating ringing after the corners.
    let deck = "kink\nV1 in 0 PULSE(0 1 1u 1u 1u 2u 10u)\nR1 in out 1k\nC1 out 0 1p\n.tran 100n 10u\n.end\n";
    let (sim, res) = run(deck);
    let br = sim.topology.soa.vsrc_branch_idx[0];
    for (k, &t) in res.time_points.iter().enumerate() {
        let i = res.waveforms[k][br];
        let exact = if t > 1.05e-6 && t < 1.95e-6 {
            -1e-6
        } else if (2.05e-6..3.95e-6).contains(&t) || (5.05e-6..9.9e-6).contains(&t) {
            0.0
        } else {
            continue;
        };
        assert!(
            (i - exact).abs() < 2e-8,
            "i(v1) at {t:e} = {i:e}, expected {exact:e}"
        );
    }
}

/// Node voltages of an `n`-section RC line (series `r`, shunt `c` to ground at every section end, open far end)
/// driven by a ramp from 0 to `v` over `tr`, at the times `ts`: classical fourth-order Runge-Kutta with a step `h`
/// far below the smallest time constant (an explicit method RustSpice does not use).
fn rc_line_rk4(n: usize, r: f64, c: f64, v: f64, tr: f64, ts: &[f64], h: f64) -> Vec<Vec<f64>> {
    let u = |t: f64| v * (t / tr).clamp(0.0, 1.0);
    let f = |t: f64, x: &[f64], dx: &mut [f64]| {
        for i in 0..n {
            let left = if i == 0 { u(t) } else { x[i - 1] };
            let right = if i + 1 < n { x[i + 1] } else { x[i] };
            dx[i] = (left - 2.0 * x[i] + right) / (r * c);
        }
    };
    let (mut x, mut t) = (vec![0.0; n], 0.0);
    let mut k = [vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    let mut tmp = vec![0.0; n];
    let mut out = Vec::with_capacity(ts.len());
    for &tg in ts {
        while t < tg - 1e-6 * h {
            let hh = h.min(tg - t);
            f(t, &x, &mut k[0]);
            for (j, (a, b)) in [(0.5, 0), (0.5, 1), (1.0, 2)].into_iter().enumerate() {
                for i in 0..n {
                    tmp[i] = x[i] + a * hh * k[b][i];
                }
                let (_, rest) = k.split_at_mut(j + 1);
                f(t + a * hh, &tmp, &mut rest[0]);
            }
            for i in 0..n {
                x[i] += hh / 6.0 * (k[0][i] + 2.0 * k[1][i] + 2.0 * k[2][i] + k[3][i]);
            }
            t += hh;
        }
        out.push(x.clone());
    }
    out
}

#[test]
fn fast_ramp_corners_keep_the_rc_line_in_phase() {
    // 60-section line (5 ohm, 10 fF: 50 fs per section) driven by a 10 ps ramp with hmax = tstep = 10 ps. The
    // ramp spans one step: first-order restart steps of hmax / 8 at its two corners used to put the diffusion front
    // ~2 ps ahead (4.5e-3 of the swing at v(22), t = 10 ps).
    let n = 60;
    let mut deck = String::from("rc line\nV1 1 0 PULSE(0 3.3 0 10p 10p 10n 20n)\n");
    for k in 1..=n {
        deck += &format!("R{k} {k} {} 5\nC{k} {} 0 10f\n", k + 1, k + 1);
    }
    deck += ".tran 10p 500p\n.end\n";
    let (sim, res) = run(&deck);
    // the corners act during the first rows: compare up to 100 ps
    let rows: Vec<usize> = (0..res.time_points.len())
        .filter(|&k| res.time_points[k] <= 100e-12 * (1.0 + 1e-9))
        .collect();
    let ts: Vec<f64> = rows.iter().map(|&k| res.time_points[k]).collect();
    let exact = rc_line_rk4(n, 5.0, 10e-15, 3.3, 10e-12, &ts, 2e-15);
    let mut worst = (0.0_f64, 0usize, 0.0_f64);
    for (j, &k) in rows.iter().enumerate() {
        for (i, &v) in exact[j].iter().enumerate() {
            let id = sim.netlist.find_node(&(i + 2).to_string()).unwrap();
            let e = (res.waveforms[k][id - 1] - v).abs() / 3.3;
            if e > worst.0 {
                worst = (e, i + 2, ts[j]);
            }
        }
    }
    assert!(
        worst.0 < 1.5e-3,
        "max error {:.3e} of the swing at v({}), t = {:e}",
        worst.0,
        worst.1,
        worst.2
    );
}

#[test]
fn tmax_bounds_the_internal_step() {
    let base = "tmax\nV1 in 0 SIN(0 1 1k)\nR1 in out 1k\nC1 out 0 1n\n";
    let (_, free) = run(&format!("{base}.tran 10u 1m\n.end\n"));
    let (_, capped) = run(&format!("{base}.tran 10u 1m 0 1u\n.end\n"));
    assert_eq!(free.time_points, capped.time_points);
    assert!(
        capped.accepted_steps >= 1000,
        "1 ms / 1 us = 1000 steps at least, got {}",
        capped.accepted_steps
    );
    assert!(free.accepted_steps < capped.accepted_steps);
}

#[test]
fn tstart_drops_the_rows_before_it() {
    let deck =
        "tstart\nV1 in 0 SIN(0 1 1Meg)\nR1 in out 1k\nC1 out 0 1n\n.tran 10n 1u 0.255u\n.end\n";
    let (_, res) = run(deck);
    assert_eq!(res.time_points[0], 0.255e-6);
    assert!((res.time_points[1] - 0.26e-6).abs() < 1e-18);
    assert_eq!(*res.time_points.last().unwrap(), 1e-6);
    assert_eq!(res.time_points.len(), 1 + 75);
}

#[test]
fn uic_starts_from_the_element_initial_conditions() {
    // C1 ic=2 discharging into 1 k (tau 1 us); L1 ic=1 A decaying into 1 ohm (tau 1 us): v(b) = -1 V at t = 0
    let deck = "uic\nR1 a 0 1k\nC1 a 0 1n ic=2\nR2 b 0 1\nL1 b 0 1u ic=1\n.tran 10n 5u uic\n.end\n";
    let (sim, res) = run(deck);
    let (a, b) = (node(&sim, &res, "a"), node(&sim, &res, "b"));
    for (k, &t) in res.time_points.iter().enumerate() {
        let decay = (-t / 1e-6).exp();
        assert!((a[k] - 2.0 * decay).abs() < 2e-3, "v(a)({t:e}) = {}", a[k]);
        assert!((b[k] + decay).abs() < 1e-3, "v(b)({t:e}) = {}", b[k]);
    }
}

#[test]
fn uic_without_initial_conditions_starts_from_zero() {
    let deck = "uic0\nV1 in 0 DC 5\nR1 in out 1k\nC1 out 0 1n\n.tran 10n 5u uic\n.end\n";
    let (sim, res) = run(deck);
    let v = node(&sim, &res, "out");
    for (k, &t) in res.time_points.iter().enumerate() {
        assert!(
            (v[k] - rc_step(t, 0.0, 5.0, 1e-6)).abs() < 5e-3,
            "v(out)({t:e}) = {}",
            v[k]
        );
    }
}

#[test]
fn spaced_dot_ic_with_uic_sets_the_capacitor() {
    let deck = "ic\nR1 out 0 1k\nC1 out 0 1n\n.ic V( out ) = 5\n.tran 10n 5u uic\n.end\n";
    let (sim, res) = run(deck);
    let v = node(&sim, &res, "out");
    for (k, &t) in res.time_points.iter().enumerate() {
        assert!(
            (v[k] - 5.0 * (-t / 1e-6).exp()).abs() < 5e-3,
            "{t:e}: {}",
            v[k]
        );
    }
}

#[test]
fn dot_ic_without_uic_is_forced_during_the_operating_point() {
    // V1 = 0, 1 k / 1 k into 1 uF, .ic v(b)=2: the t = 0 solution has v(a) = 1 V (consistent with the forced
    // node), then v(b) = 2 exp(-t / 2 ms) and v(a) = v(b) / 2.
    let deck =
        "ic op\nV1 in 0 DC 0\nR1 in a 1k\nR2 a b 1k\nC1 b 0 1u\n.ic v(b)=2\n.tran 20u 3m\n.end\n";
    let (mut sim, res) = run(deck);
    let (a, b) = (node(&sim, &res, "a"), node(&sim, &res, "b"));
    assert!((a[0] - 1.0).abs() < 1e-6, "v(a)(0) = {}", a[0]);
    assert!((b[0] - 2.0).abs() < 1e-6, "v(b)(0) = {}", b[0]);
    for (k, &t) in res.time_points.iter().enumerate() {
        let vb = 2.0 * (-t / 2e-3).exp();
        assert!((b[k] - vb).abs() < 2e-3, "v(b)({t:e}) = {}", b[k]);
        assert!((a[k] - vb / 2.0).abs() < 1e-3, "v(a)({t:e}) = {}", a[k]);
    }
    // the clamp is released: a later .op of the same simulator sees no trace of it
    let op = sim.run_dc_op().unwrap();
    assert!(op.solution.iter().take(3).all(|v| v.abs() < 1e-9));
}

#[test]
fn smooth_linear_drive_keeps_one_factorisation() {
    // tstep far below the time constant: every step is the full tstep after the start-up, so the constant-h
    // LU must be reused instead of refactorised per step.
    let deck = "reuse\nV1 in 0 SIN(0 1 5k)\nR1 in out 1k\nC1 out 0 10n\n.tran 200n 2m\n.end\n";
    let (_, res) = run(deck);
    assert_eq!(res.time_points.len(), 10_001);
    assert!(
        res.lu_factorizations < 40,
        "{} factorizations for {} steps",
        res.lu_factorizations,
        res.accepted_steps
    );
    assert!(res.accepted_steps < 10_050, "{} steps", res.accepted_steps);
}

#[test]
fn breakpoints_of_a_periodic_pulse() {
    use rustspice_core::netlist::SourceWaveform;
    let w = SourceWaveform::Pulse {
        dc_val: 0.0,
        v1: 0.0,
        v2: 1.0,
        td: 1e-6,
        tr: 1e-7,
        tf: 2e-7,
        pw: 3e-7,
        per: 1e-6,
    };
    let mut t = 0.0;
    let mut got = Vec::new();
    while let Some(bp) = w.next_breakpoint(t) {
        if bp > 3.05e-6 {
            break;
        }
        got.push(bp);
        t = bp;
    }
    let want = [
        1.0e-6, 1.1e-6, 1.4e-6, 1.6e-6, 2.0e-6, 2.1e-6, 2.4e-6, 2.6e-6, 3.0e-6,
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-18, "{got:?}");
    }
    let sin = SourceWaveform::Sin {
        dc_val: 0.0,
        vo: 0.0,
        va: 1.0,
        freq: 1e6,
        td: 5e-7,
        theta: 0.0,
        phase: 0.0,
    };
    assert_eq!(sin.next_breakpoint(0.0), Some(5e-7));
    assert_eq!(sin.next_breakpoint(5e-7), None);
    assert_eq!(SourceWaveform::Dc(1.0).next_breakpoint(0.0), None);
}
