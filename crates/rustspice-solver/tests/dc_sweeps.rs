//! Nested `.dc`, resistor and temperature sweeps, and `.nodeset`. Expected values are closed forms of the
//! circuits (voltage dividers, `R(T) = R (1 + tc1 dT)`) and the logic state of a latch, not solver output.

use rustspice_core::{AnalysisCmd, DcSweep};
use rustspice_solver::Simulator;

fn dc_card(sim: &Simulator) -> (DcSweep, Option<DcSweep>) {
    match &sim.netlist.analyses[0] {
        AnalysisCmd::Dc {
            source_name,
            start,
            stop,
            step,
            outer,
        } => (
            DcSweep {
                name: source_name.clone(),
                start: *start,
                stop: *stop,
                step: *step,
            },
            outer.clone(),
        ),
        other => panic!("{other:?}"),
    }
}

fn v(sim: &Simulator, x: &[f64], node: &str) -> f64 {
    sim.node_voltage(x, node).unwrap()
}

#[test]
fn nested_sweep_runs_the_first_source_fastest() {
    // out = (v(a) + v(b)) / 3 for three equal resistors into one node
    let mut sim = Simulator::from_spice(
        "nested\nV1 a 0 0\nV2 b 0 0\nR1 a out 1k\nR2 b out 1k\nR3 out 0 1k\n.dc V1 0 2 1 V2 0 1 0.5\n.end\n",
    )
    .unwrap();
    let (inner, outer) = dc_card(&sim);
    let r = sim.run_dc_analysis(&inner, outer.as_ref()).unwrap();
    assert_eq!(
        r.sweep_values,
        vec![0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0]
    );
    for (k, x) in r.solutions.iter().enumerate() {
        let (va, vb) = ((k % 3) as f64, (k / 3) as f64 * 0.5);
        assert!(
            (v(&sim, x, "out") - (va + vb) / 3.0).abs() < 1e-12,
            "point {k}"
        );
    }
    // the deck's own source values are back afterwards
    let op = sim.run_dc_op().unwrap();
    assert!(v(&sim, &op.solution, "out").abs() < 1e-12);
}

#[test]
fn resistor_sweep_sets_the_value_and_restores_it() {
    let mut sim = Simulator::from_spice(
        "rsweep\nV1 in 0 6\nR1 in out 1k\nR2 out 0 1k\n.dc R1 1k 3k 1k\n.end\n",
    )
    .unwrap();
    let (inner, outer) = dc_card(&sim);
    let r = sim.run_dc_analysis(&inner, outer.as_ref()).unwrap();
    assert_eq!(r.sweep_values, vec![1e3, 2e3, 3e3]);
    for (x, rv) in r.solutions.iter().zip([1e3, 2e3, 3e3]) {
        assert!((v(&sim, x, "out") - 6.0 * 1e3 / (rv + 1e3)).abs() < 1e-12);
    }
    let op = sim.run_dc_op().unwrap();
    assert!((v(&sim, &op.solution, "out") - 3.0).abs() < 1e-12);
}

#[test]
fn temperature_sweep_reevaluates_linear_coefficients() {
    // R1(T) = 1k (1 + 0.01 (T - 27))
    let mut sim = Simulator::from_spice(
        "tsweep\nV1 in 0 6\nR1 in out 1k tc1=0.01\nR2 out 0 1k\n.dc temp 27 127 50\n.end\n",
    )
    .unwrap();
    let (inner, outer) = dc_card(&sim);
    let r = sim.run_dc_analysis(&inner, outer.as_ref()).unwrap();
    assert_eq!(r.sweep_values, vec![27.0, 77.0, 127.0]);
    for (x, t) in r.solutions.iter().zip([27.0, 77.0, 127.0]) {
        let r1 = 1e3 * (1.0 + 0.01 * (t - 27.0));
        assert!(
            (v(&sim, x, "out") - 6.0 * 1e3 / (r1 + 1e3)).abs() < 1e-12,
            "T = {t}"
        );
    }
    let op = sim.run_dc_op().unwrap();
    assert!((v(&sim, &op.solution, "out") - 3.0).abs() < 1e-12);
    assert_eq!(sim.netlist.temp, None);
}

#[test]
fn unknown_sweep_variable_is_refused() {
    let mut sim =
        Simulator::from_spice("bad\nV1 in 0 6\nR1 in 0 1k\n.dc C9 0 1 1\n.end\n").unwrap();
    let (inner, outer) = dc_card(&sim);
    let e = sim.run_dc_analysis(&inner, outer.as_ref()).unwrap_err();
    assert!(e.to_string().contains("C9"), "{e}");
}

const LATCH: &str = "latch\nVDD vdd 0 3\n\
    M1 q qb 0 0 NM W=10u L=1u\nM2 q qb vdd vdd PM W=20u L=1u\n\
    M3 qb q 0 0 NM W=10u L=1u\nM4 qb q vdd vdd PM W=20u L=1u\n\
    .model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)\n.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u)\n";

#[test]
fn nodeset_selects_the_state_of_a_bistable_latch() {
    for (q0, qb0, q_high) in [(3.0, 0.0, true), (0.0, 3.0, false)] {
        let deck = format!("{LATCH}.nodeset v(q)={q0} v(qb)={qb0}\n.op\n.end\n");
        let mut sim = Simulator::from_spice(&deck).unwrap();
        let x = sim.run_dc_op().unwrap().solution;
        let (q, qb) = (v(&sim, &x, "q"), v(&sim, &x, "qb"));
        // a settled latch sits at the rails, and the nodeset is released (no residual clamp)
        let (hi, lo) = if q_high { (q, qb) } else { (qb, q) };
        assert!(
            hi > 2.999 && lo < 1e-3,
            "nodeset q={q0}: q = {q}, qb = {qb}"
        );
    }
}
