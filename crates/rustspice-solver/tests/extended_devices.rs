//! Controlled sources (`E`, `F`, `H`, `POLY(1)`), coupled inductors (`K`), semiconductor `R` / `C` models, full diode
//! junctions (series resistance, breakdown, graded capacitance) and level-1 MOSFET process parameters, series
//! resistances and bulk junctions.
//!
//! Every expected value is a closed form, a scalar root of the circuit's KCL or an RK4 integration written here (not a
//! RustSpice run). The differential check against black-box ngspice is the `cap_el_*` / `cap_dev_*` benchmark decks.

use rustspice_core::netlist::AnalysisCmd;
use rustspice_solver::{Simulator, TranResult, TranSpec};

/// kT/q at 27 °C.
const VT: f64 = 0.025864917007155805;
const GMIN: f64 = 1e-12;

fn op(deck: &str) -> (Simulator, Vec<f64>) {
    let mut sim = Simulator::from_spice(deck).unwrap();
    let x = sim.run_dc_op().unwrap().solution;
    (sim, x)
}

fn v(sim: &Simulator, x: &[f64], node: &str) -> f64 {
    x[sim.netlist.find_node(node).unwrap() - 1]
}

/// Branch current of a voltage source / `E` / `H` (current into its `+` terminal, SPICE convention).
fn i(sim: &Simulator, x: &[f64], name: &str) -> f64 {
    let soa = &sim.topology.soa;
    let br = soa
        .vsrc_names
        .iter()
        .zip(&soa.vsrc_branch_idx)
        .chain(soa.ctl_names.iter().zip(&soa.ctl_branch_idx))
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, &b)| b)
        .unwrap();
    x[br]
}

fn tran(deck: &str) -> (Simulator, TranResult) {
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
        .unwrap();
    let res = sim.run_transient_spec(&spec).unwrap();
    (sim, res)
}

/// Root of a monotone `f` on `[lo, hi]` by bisection.
fn bisect(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let flo = f(lo);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if (f(mid) > 0.0) == (flo > 0.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn linear_controlled_sources() {
    let (sim, x) = op("e\nV1 in 0 2\nE1 out 0 in 0 3\nR1 out 0 1k\n.op\n.end\n");
    assert!((v(&sim, &x, "out") - 6.0).abs() < 1e-12);
    // the source delivers 6 mA into R1: -6 mA flows into its + terminal
    assert!((i(&sim, &x, "e1") + 6e-3).abs() < 1e-15);

    // i(V1) = -1 mA; F pushes 10 * i(V1) from node 0 through itself into `out`
    let (sim, x) = op("f\nV1 in 0 1\nR1 in 0 1k\nF1 0 out V1 10\nR2 out 0 1k\n.op\n.end\n");
    assert!((v(&sim, &x, "out") + 10.0).abs() < 1e-12);

    let (sim, x) = op("h\nV1 in 0 1\nR1 in 0 1k\nH1 out 0 V1 2k\nR2 out 0 1k\n.op\n.end\n");
    assert!((v(&sim, &x, "out") + 2.0).abs() < 1e-12);
}

#[test]
fn polynomial_sources() {
    // the control voltage comes from a divider: x = 2 V, v = 1 + 2 x + 3 x^2 = 17 V
    let (sim, x) = op(
        "e poly\nV1 in 0 3\nR1 in c 1k\nR2 c 0 2k\nE1 out 0 POLY(1) c 0 1 2 3\nR3 out 0 1k\n.op\n.end\n",
    );
    assert!((v(&sim, &x, "out") - 17.0).abs() < 1e-9);
    // i = 1m x + 0.5m x^2 = 4 mA enters `out` (the source points from 0 to out)
    let (sim, x) =
        op("g poly\nV1 in 0 2\nG1 0 out poly(1) in 0 0 1m 0.5m\nR1 out 0 1k\n.op\n.end\n");
    assert!((v(&sim, &x, "out") - 4.0).abs() < 1e-9);
    // SPICE: a single POLY(1) coefficient is the linear term
    let (sim, x) = op("e1\nV1 in 0 2\nE1 out 0 poly(1) in 0 5\nR1 out 0 1k\n.op\n.end\n");
    assert!((v(&sim, &x, "out") - 10.0).abs() < 1e-12);
}

#[test]
fn controlled_sources_inside_subcircuits_refer_to_local_elements() {
    let deck = "sub\n.subckt amp a y\nVs a m 0\nRm m 0 1k\nH1 y 0 Vs 3k\nRy y 0 1k\n.ends\nV1 in 0 1\nX1 in out amp\n.op\n.end\n";
    let (sim, x) = op(deck);
    // i(Vs) = 1 mA into its + terminal, v(out) = 3k * 1 mA
    assert!((v(&sim, &x, "out") - 3.0).abs() < 1e-12);
}

#[test]
fn unknown_control_source_is_refused() {
    let err =
        Simulator::from_spice("f\nV1 in 0 1\nR1 in 0 1k\nF1 0 out VX 10\nR2 out 0 1k\n.op\n.end\n")
            .err()
            .unwrap();
    assert!(err.to_string().contains("VX"), "{err}");
    assert!(Simulator::from_spice("k\nL1 a 0 1u\nR1 a 0 1\nK1 L1 L9 0.5\n.op\n.end\n").is_err());
}

#[test]
fn coupled_inductors_transfer_the_primary_rate() {
    // uic: i1(0) = 0, i1 = (1 - exp(-t / tau)) A with tau = L1 / R1 = 1 us; the secondary is loaded by 1 Mohm
    // (its current, ~1e-6 of the primary's, is ignored), so v(sec) = M di1/dt = k exp(-t / tau)
    let deck = "k\nV1 in 0 1\nR1 in p 1\nL1 p 0 1u\nL2 sec 0 1u\nK1 L1 L2 0.9\nR2 sec 0 1meg\n.tran 10n 3u uic\n.end\n";
    let (sim, res) = tran(deck);
    let id = sim.netlist.find_node("sec").unwrap();
    for (k, &t) in res.time_points.iter().enumerate().skip(1) {
        let want = 0.9 * (-t / 1e-6).exp();
        let got = res.waveforms[k][id - 1];
        assert!((got - want).abs() < 2e-3, "t={t:e}: {got} vs {want}");
    }
}

#[test]
fn semiconductor_resistor_and_capacitor_models() {
    let (sim, x) = op(
        "r\nV1 a 0 1\n.model rm R(rsh=100 narrow=1u short=2u)\nR1 a 0 rm w=12u l=100u\n.op\n.end\n",
    );
    // R = RSH (L - 2 SHORT) / (W - 2 NARROW) = 960 ohm
    assert!((i(&sim, &x, "v1") + 1.0 / 960.0).abs() < 1e-15);
    let n = rustspice_core::parse_netlist(
        "c\nV1 a 0 1\n.model cm C(cj=1e-3 cjsw=1e-9 narrow=1u short=2u)\nC1 a 0 cm w=10u l=100u\n.model cm2 C(cj=1e-3)\nC2 a 0 cm2 l=100u\n.op\n.end\n",
    )
    .unwrap();
    // CJ W' L' + 2 CJSW (W' + L') with W' = W - NARROW, L' = L - SHORT; default width 10 um
    assert!((n.capacitors[0].capacitance - 1.096e-12).abs() < 1e-24);
    assert!((n.capacitors[1].capacitance - 1e-12).abs() < 1e-24);
}

#[test]
fn diode_series_resistance() {
    let (sim, x) =
        op("d\nV1 in 0 5\nR1 in a 100\nD1 a 0 dm\n.model dm D(IS=1e-14 RS=20)\n.op\n.end\n");
    let is = 1e-14;
    // KCL: (5 - va) / 100 = I with va = vj + 20 I and I = IS (exp(vj / VT) - 1) + GMIN vj
    let vj = bisect(0.3, 1.0, |vj| {
        let id = is * ((vj / VT).exp() - 1.0) + GMIN * vj;
        (5.0 - vj - 20.0 * id) / 100.0 - id
    });
    let id = is * ((vj / VT).exp() - 1.0) + GMIN * vj;
    assert!((v(&sim, &x, "a") - (vj + 20.0 * id)).abs() < 1e-9);
    // the internal node is not an output
    assert!(sim.netlist.find_node("d1#internal").is_none());
}

#[test]
fn diode_breakdown_reaches_ibv_at_bv() {
    let (sim, x) = op(
        "bv\nV1 in 0 -10\nR1 in d 1k\nD1 d 0 dm\n.model dm D(IS=1e-14 N=1 BV=5 IBV=1m)\n.op\n.end\n",
    );
    // breakdown branch: i = -IBV exp(-(v + BV) / VT) (the knee is fitted so that i(-BV) = -IBV) + GMIN v
    let vd = bisect(-6.0, -4.5, |vd| {
        let id = -1e-3 * (-(vd + 5.0) / VT).exp() + GMIN * vd;
        (-10.0 - vd) / 1e3 - id
    });
    assert!(
        (v(&sim, &x, "d") - vd).abs() < 1e-8,
        "{} vs {vd}",
        v(&sim, &x, "d")
    );
}

/// RK4 of `dv/dt = (vin - v) / (R C(v))` for a reverse-biased junction `C(v) = CJO / (1 - v / VJ)^M` (`v <= 0`).
fn junction_rc(vin: f64, r: f64, cjo: f64, vj: f64, m: f64, ts: &[f64], h: f64) -> Vec<f64> {
    let f = |v: f64| (vin - v) / (r * cjo * (1.0 - v / vj).powf(-m));
    let (mut v, mut t) = (0.0_f64, 0.0_f64);
    let mut out = Vec::with_capacity(ts.len());
    for &tg in ts {
        while t < tg - 1e-6 * h {
            let hh = h.min(tg - t);
            let k1 = f(v);
            let k2 = f(v + 0.5 * hh * k1);
            let k3 = f(v + 0.5 * hh * k2);
            let k4 = f(v + hh * k3);
            v += hh / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
            t += hh;
        }
        out.push(v);
    }
    out
}

#[test]
fn graded_junction_capacitance_charges_like_its_ode() {
    // uic: the junction starts at 0 V and charges towards -3 V through 10k (its 1e-14 A leakage is negligible)
    let deck =
        "cj\nV1 in 0 -3\nR1 in a 10k\nD1 a 0 dm\n.model dm D(IS=1e-14 CJO=2p VJ=0.7 M=0.4)\n.tran 0.5n 100n uic\n.end\n";
    let (sim, res) = tran(deck);
    let id = sim.netlist.find_node("a").unwrap();
    let exact = junction_rc(-3.0, 1e4, 2e-12, 0.7, 0.4, &res.time_points, 1e-12);
    for (k, &t) in res.time_points.iter().enumerate() {
        let got = res.waveforms[k][id - 1];
        assert!(
            (got - exact[k]).abs() < 5e-3 * 3.0,
            "t={t:e}: {got} vs {}",
            exact[k]
        );
    }
    // a constant 2 pF would be ~0.2 V away at one time constant: the grading is in effect
    let k = res.time_points.iter().position(|&t| t >= 20e-9).unwrap();
    let constant = -3.0 * (1.0 - (-res.time_points[k] / 20e-9).exp());
    assert!((res.waveforms[k][id - 1] - constant).abs() > 0.1);
}

/// Saturation drain current of a level-1 NMOS without channel-length modulation.
fn id_sat(kp: f64, w: f64, l: f64, vov: f64) -> f64 {
    0.5 * kp * w / l * vov * vov
}

/// Reverse current of the default (`IS` = 1e-14 A) drain-bulk junction of an NMOS whose bulk is grounded, GMIN
/// included.
fn drain_leak(vd: f64) -> f64 {
    1e-14 * (1.0 - (-vd / VT).exp()) + GMIN * vd
}

#[test]
fn level1_kp_from_mobility_and_oxide() {
    let deck = "m\nVg g 0 2\nVd d 0 3\nM1 d g 0 0 nm w=10u l=1u\n.model nm nmos(level=1 vto=0.7 uo=600 tox=1e-8)\n.op\n.end\n";
    let (sim, x) = op(deck);
    let kp = 600e-4 * 3.9 * 8.854214871e-12 / 1e-8;
    let want = id_sat(kp, 10e-6, 1e-6, 1.3) + drain_leak(3.0);
    assert!((i(&sim, &x, "vd") + want).abs() < 1e-12 * want);
    // UO without TOX is not a transconductance: KP stays at its 2e-5 default
    let (sim, x) = op(&deck.replace(" tox=1e-8", ""));
    let want = id_sat(2e-5, 10e-6, 1e-6, 1.3) + drain_leak(3.0);
    assert!((i(&sim, &x, "vd") + want).abs() < 1e-12 * want);
}

#[test]
fn level1_threshold_from_substrate_doping() {
    let deck = "m\nVg g 0 2\nVd d 0 3\nM1 d g 0 0 nm w=10u l=1u\n.model nm nmos(level=1 kp=1e-4 nsub=1e16 tox=1e-8)\n.op\n.end\n";
    let (sim, x) = op(deck);
    // textbook derivation at 300.15 K: PHI = 2 VT ln(NSUB / ni), GAMMA = sqrt(2 q eps_si NSUB) / Cox,
    // VTO = -(Eg / 2 + PHI / 2) + PHI + GAMMA sqrt(PHI) for an n+ poly gate (TPG = 1)
    let eps0 = 8.854214871e-12_f64;
    let cox = 3.9 * eps0 / 1e-8;
    let phi = 2.0 * VT * (1e22_f64 / 1.45e16).ln();
    let gamma = (2.0 * 1.602176634e-19 * 11.7 * eps0 * 1e22).sqrt() / cox;
    let t = 300.15;
    let eg = 1.16 - 7.02e-4 * t * t / (t + 1108.0);
    let vto = -(0.5 * eg + 0.5 * phi) + phi + gamma * phi.sqrt();
    assert!((vto + 0.070677).abs() < 1e-5, "{vto}");
    let want = id_sat(1e-4, 10e-6, 1e-6, 2.0 - vto) + drain_leak(3.0);
    assert!((i(&sim, &x, "vd") + want).abs() < 1e-8 * want);
}

#[test]
fn level1_lateral_diffusion_and_drain_resistance() {
    let base = "m\nVg g 0 2\nVd d 0 3\nM1 d g 0 0 nm w=10u l=1u\n.model nm nmos(level=1 vto=0.7 kp=1e-4 ld=0.1u)\n.op\n.end\n";
    let (sim, x) = op(base);
    let want = id_sat(1e-4, 10e-6, 0.8e-6, 1.3) + drain_leak(3.0);
    assert!((i(&sim, &x, "vd") + want).abs() < 1e-12 * want);

    // RD = 200 ohm, triode: the internal drain vds' carries the channel current KP W/L (vov vds' - vds'^2 / 2) and
    // the junction leakage, both fed through RD
    let deck = "m\nVg g 0 3\nVd d 0 0.5\nM1 d g 0 0 nm w=10u l=1u\n.model nm nmos(level=1 vto=0.7 kp=1e-4 rd=200)\n.op\n.end\n";
    let (sim, x) = op(deck);
    let beta = 1e-4 * 10.0;
    let inner = |vds: f64| beta * (2.3 * vds - 0.5 * vds * vds) + drain_leak(vds);
    let vds = bisect(0.0, 0.5, |vds| (0.5 - vds) / 200.0 - inner(vds));
    let want = inner(vds);
    assert!(
        (i(&sim, &x, "vd") + want).abs() < 1e-12 * want,
        "{} vs {want}",
        -i(&sim, &x, "vd")
    );
}

#[test]
fn level1_bulk_junctions_conduct_when_forward_biased() {
    // the bulk is driven to 1.5 V through 1k: the bulk-source junction clamps it (the channel stays off)
    let deck = "m\nVb vb 0 1.5\nRb vb b 1k\nVd d 0 1\nVg g 0 0\nM1 d g 0 b nm w=20u l=1u\n.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7)\n.op\n.end\n";
    let (sim, x) = op(deck);
    let j = |v: f64| 1e-14 * ((v / VT).exp() - 1.0) + GMIN * v;
    let vb = bisect(0.3, 1.0, |vb| (1.5 - vb) / 1e3 - j(vb) - j(vb - 1.0));
    assert!(
        (v(&sim, &x, "b") - vb).abs() < 1e-9,
        "{} vs {vb}",
        v(&sim, &x, "b")
    );
}

#[test]
fn level1_meyer_gate_capacitance_in_accumulation() {
    // the gate is pulled from 0 V to -3 V with source, drain and bulk grounded: vgs <= VTO - PHI throughout, where
    // Meyer puts the whole oxide capacitance Cox W L between gate and bulk (no overlap capacitances are given)
    let deck = "meyer\nV1 in 0 -3\nR1 in g 10k\nM1 0 g 0 0 nm w=100u l=10u\n.model nm nmos(level=1 vto=0.7 kp=1e-4 phi=0.7 tox=1e-8)\n.tran 0.5n 150n uic\n.end\n";
    let (sim, res) = tran(deck);
    let id = sim.netlist.find_node("g").unwrap();
    let tau = 1e4 * 3.9 * 8.854214871e-12 / 1e-8 * 100e-6 * 10e-6;
    for (k, &t) in res.time_points.iter().enumerate() {
        let want = -3.0 * (1.0 - (-t / tau).exp());
        let got = res.waveforms[k][id - 1];
        assert!((got - want).abs() < 1e-2, "t={t:e}: {got} vs {want}");
    }
}
