//! Time-grid / sweep-grid regression tests: integer step counting for `.tran`, integer point counting
//! for `.dc`, and the Backward-Euler start-up sub-steps of the first transient interval.
//!
//! The expected row counts follow `floor(tstop / tstep + 1e-9) + 1` (+1 for a genuinely shorter last
//! step); they do not depend on any simulator, so these tests are not circular.

use rustspice_solver::Simulator;

fn rc_deck(tran: &str) -> String {
    format!("RC grid test\nV1 in 0 SIN(0 1 1Meg)\nR1 in out 1k\nC1 out 0 1n\n{tran}\n.end\n")
}

#[test]
fn tran_exact_multiple_has_no_sliver_row() {
    // 100u / 10n = 10000 steps. The old accumulated `t += h` ended 1.3e-17 s short of tstop (> 1e-9 * tstep)
    // and appended a 1.3e-17 s step as row 10002.
    let mut sim = Simulator::from_spice(&rc_deck(".tran 10n 100u")).unwrap();
    let res = sim.run_transient(10e-9, 100e-6).unwrap();
    assert_eq!(res.time_points.len(), 10_001);
    assert_eq!(res.waveforms.len(), 10_001);
    assert_eq!(*res.time_points.last().unwrap(), 100e-6);
    for (k, &t) in res.time_points.iter().enumerate().take(10_000) {
        assert!(
            (t - (k as f64) * 10e-9).abs() <= 1e-24,
            "row {k}: t = {t:e} is off the k * tstep grid"
        );
    }
    let min_dt = res
        .time_points
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(f64::INFINITY, f64::min);
    assert!(min_dt > 0.99e-8, "smallest dt = {min_dt:e}");
}

#[test]
fn tran_partial_last_step_and_step_larger_than_stop() {
    // 105n / 10n: ten regular rows after t = 0 plus one 5 ns tail row.
    let mut sim = Simulator::from_spice(&rc_deck(".tran 10n 105n")).unwrap();
    let res = sim.run_transient(10e-9, 105e-9).unwrap();
    assert_eq!(res.time_points.len(), 12);
    assert!((res.time_points[10] - 100e-9).abs() < 1e-18);
    assert!((res.time_points[11] - 105e-9).abs() < 1e-18);

    // tstep > tstop: a single step of length tstop.
    let mut sim = Simulator::from_spice(&rc_deck(".tran 1u 300n")).unwrap();
    let res = sim.run_transient(1e-6, 300e-9).unwrap();
    assert_eq!(res.time_points.len(), 2);
    assert!((res.time_points[1] - 300e-9).abs() < 1e-18);

    // A tail far below the 1e-9-step tolerance (1e-11 steps) is merged into the last regular step.
    let mut sim = Simulator::from_spice(&rc_deck(".tran 10n 100.0000000001n")).unwrap();
    let res = sim.run_transient(10e-9, 100.0000000001e-9).unwrap();
    assert_eq!(res.time_points.len(), 11);
    assert_eq!(*res.time_points.last().unwrap(), 100.0000000001e-9);
}

#[test]
fn tran_first_interval_error_is_small() {
    // RC, tau = 1 us, tstep = 100 ns (a = 0.1), source ramp (1 ns) inside the first interval.
    // One full-tstep Backward-Euler step gave v(100 ns) = a / (1 + a) = 0.090909 (exact 0.094710, error 3.8e-3).
    let deck = "RC first step\nV1 in 0 PULSE(0 1 0 1n 1n 1 2)\nR1 in out 1k\nC1 out 0 1n\n.tran 100n 5u\n.end\n";
    let mut sim = Simulator::from_spice(deck).unwrap();
    let res = sim.run_transient(100e-9, 5e-6).unwrap();
    assert_eq!(res.time_points.len(), 51);
    let v1 = sim.node_voltage(&res.waveforms[1], "out").unwrap();
    // Closed form for a 1 ns ramp ending long before tau: v(t) = 1 - exp(-t/tau) + O(1e-3 * t/tau)
    let exact = 1.0 - (-0.1_f64).exp();
    assert!(
        (v1 - exact).abs() < 1e-3,
        "v(100 ns) = {v1}, exact step response {exact}"
    );
}

#[test]
fn dc_sweep_decimal_steps_keep_the_last_point() {
    let deck = "DC grid\nV1 in 0 0\nR1 in out 2k\nR2 out 0 1k\n.dc V1 0 5 1m\n.end\n";
    let mut sim = Simulator::from_spice(deck).unwrap();
    // (start, stop, step, expected points = floor(|stop - start| / |step| + 1e-9) + 1)
    let cases: [(f64, f64, f64, usize); 9] = [
        (0.0, 5.0, 1e-3, 5001),
        (5.0, 0.0, -1e-3, 5001),
        (-5.0, 5.0, 1e-3, 10_001),
        (1.0, 6.0, 1e-3, 5001),
        (0.0, 100.0, 1e-2, 10_001),
        (0.0, 0.3, 0.1, 4),
        (0.0, 1.0, 0.3, 4),
        (2.0, 2.0, 1.0, 1),
        (0.0, 4.0, 1e-3, 4001),
    ];
    for (a, b, s, n) in cases {
        let res = sim.run_dc_sweep("V1", a, b, s).unwrap();
        assert_eq!(res.sweep_values.len(), n, "sweep {a} {b} {s}");
        assert_eq!(res.solutions.len(), n);
        assert_eq!(res.sweep_values[0], a);
        let last = *res.sweep_values.last().unwrap();
        let k = (n - 1) as f64;
        assert!(
            (last - (a + k * s)).abs() <= 1e-12 * (1.0 + a.abs() + b.abs()),
            "last sweep value {last} for {a} {b} {s}"
        );
    }
    // 1 mV steps: abscissae stay within accumulated-rounding distance of `k * step` (4.4e-15 at 5 V).
    let res = sim.run_dc_sweep("V1", 0.0, 5.0, 1e-3).unwrap();
    for (k, &v) in res.sweep_values.iter().enumerate() {
        assert!((v - (k as f64) * 1e-3).abs() <= 1e-12, "point {k}: {v}");
    }
}

/// Linear-interpolated crossings of `level` between the rows.
fn crossings(t: &[f64], y: &[f64], level: f64) -> Vec<f64> {
    (1..t.len())
        .filter_map(|k| {
            let (a, b) = (y[k - 1] - level, y[k] - level);
            ((a < 0.0 && b >= 0.0) || (a > 0.0 && b <= 0.0))
                .then(|| t[k - 1] + a / (a - b) * (t[k] - t[k - 1]))
        })
        .collect()
}

#[test]
fn internal_points_resolve_an_edge_shorter_than_tstep() {
    // RC (tau = 5 ps) driven by a 10 ps ramp to 1 V, rows every 10 ps. Closed form: the ramp response
    // g(t) = (t - tau (1 - exp(-t / tau))) / tr, and v(t) = g(t) - g(t - tr) after the ramp.
    let deck =
        "edge\nV1 in 0 PULSE(0 1 0 10p 10p 1n 2n)\nR1 in out 1k\nC1 out 0 5f\n.tran 10p 1n\n.end\n";
    let (tau, tr) = (5e-12, 10e-12);
    let g = |t: f64| {
        if t <= 0.0 {
            0.0
        } else {
            (t - tau * (1.0 - (-t / tau).exp())) / tr
        }
    };
    let v = |t: f64| g(t) - g(t - tr);
    let (mut lo, mut hi) = (0.0, 100e-12);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if v(mid) < 0.5 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let exact = 0.5 * (lo + hi);

    let run = |internal: bool| {
        let mut sim = Simulator::from_spice(deck).unwrap();
        sim.transient_solver.options.internal_points = internal;
        let res = sim.run_transient(10e-12, 1e-9).unwrap();
        let out = sim.netlist.find_node("out").unwrap() - 1;
        let y: Vec<f64> = res.waveforms.iter().map(|w| w[out]).collect();
        (res.time_points, y)
    };
    let (t_grid, y_grid) = run(false);
    let (t_all, y_all) = run(true);

    // the grid rows are written either way, with the same values; the extra rows lie strictly between them
    let on_grid: Vec<usize> = (0..t_all.len())
        .filter(|&i| t_grid.contains(&t_all[i]))
        .collect();
    assert_eq!(on_grid.len(), t_grid.len());
    for (k, &i) in on_grid.iter().enumerate() {
        assert_eq!(y_all[i].to_bits(), y_grid[k].to_bits(), "row {k}");
    }
    assert!(t_all.len() > t_grid.len());
    assert!(t_all.windows(2).all(|w| w[1] > w[0]));

    let c_grid = crossings(&t_grid, &y_grid, 0.5);
    let c_all = crossings(&t_all, &y_all, 0.5);
    assert_eq!((c_grid.len(), c_all.len()), (1, 1));
    // 10 ps rows cannot place a crossing that lies inside a 10 ps edge; the accepted steps can
    assert!(
        (c_grid[0] - exact).abs() > 0.3e-12,
        "grid {:e} vs {exact:e}",
        c_grid[0]
    );
    assert!(
        (c_all[0] - exact).abs() < 0.05e-12,
        "internal {:e} vs {exact:e}",
        c_all[0]
    );
}
