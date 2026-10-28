//! Point-count budgets of `.dc` / `.tran` and the non-finite solution guard (AGENTS.md Rule 9).
//! Expected point counts come from the closed forms `floor(tstop/tstep)+1` and `floor((stop-start)/step)+1`.

use rustspice_core::{ResourceLimits, SimError};
use rustspice_solver::Simulator;

const RC: &str = "rc\nV1 1 0 PULSE(0 5 0 1n 1n 1 2)\nR1 1 2 1k\nC1 2 0 1n\n.op\n.end\n";
const DIV: &str = "div\nV1 1 0 5\nR1 1 2 1k\nR2 2 0 1k\n.op\n.end\n";

fn sim_with(deck: &str, f: impl FnOnce(&mut ResourceLimits)) -> Simulator {
    let mut sim = Simulator::from_spice(deck).unwrap();
    let mut lim = ResourceLimits::default();
    f(&mut lim);
    sim.set_limits(lim);
    sim
}

fn resource(e: SimError) -> &'static str {
    match e {
        SimError::ResourceLimit { resource, .. } => resource,
        other => panic!("expected ResourceLimit, got {other:?}"),
    }
}

#[test]
fn tran_point_budget_refuses_before_allocating() {
    // 1e12 points would be an 8 TB reservation in the former `Vec::with_capacity`; 1e300 saturated usize
    for (tstep, tstop) in [(1e-12, 1.0), (1e-9, 1000.0), (1.0, 1e300), (1e-9, 1.0)] {
        let mut sim = sim_with(RC, |_| {});
        let t0 = std::time::Instant::now();
        let e = sim.run_transient(tstep, tstop).expect_err("budget");
        assert_eq!(resource(e), "transient_points");
        assert!(
            t0.elapsed().as_secs_f64() < 1.0,
            "refusal must not do the work"
        );
    }
}

#[test]
fn tran_nonfinite_or_nonpositive_bounds_are_refused() {
    for (tstep, tstop) in [
        (1e-9, f64::INFINITY),
        (f64::NAN, 1.0),
        (1e-9, f64::NAN),
        (0.0, 1.0),
        (1e-9, -1.0),
    ] {
        let mut sim = sim_with(RC, |_| {});
        assert!(matches!(
            sim.run_transient(tstep, tstop),
            Err(SimError::TransientFailure { .. })
        ));
    }
}

#[test]
fn tran_within_budget_returns_the_closed_form_row_count() {
    // tstep 0.5 s, tstop 64 s (ratio exactly 128): floor(64 / 0.5) + 1 = 129 rows; the estimate is
    // ceil(128) + 2 = 130, so max_points = 130 admits it and 129 does not (conservative by design)
    let mut sim = sim_with(RC, |l| l.max_points = 130);
    let r = sim.run_transient(0.5, 64.0).expect("fits");
    assert_eq!(r.time_points.len(), 129);
    assert_eq!(r.waveforms.len(), 129);
    let mut sim = sim_with(RC, |l| l.max_points = 129);
    assert_eq!(
        resource(sim.run_transient(0.5, 64.0).unwrap_err()),
        "transient_points"
    );
}

#[test]
fn result_byte_budget_scales_with_dimension() {
    let dim = Simulator::from_spice(RC).unwrap().topology.soa.mna_dim;
    assert_eq!(dim, 3);
    let per = ResourceLimits::bytes_per_point(dim);
    assert_eq!(per, 72);
    let mut sim = sim_with(RC, |l| l.max_result_bytes = 130 * per);
    assert_eq!(sim.run_transient(0.5, 64.0).unwrap().time_points.len(), 129);
    let mut sim = sim_with(RC, |l| l.max_result_bytes = 129 * per);
    assert_eq!(
        resource(sim.run_transient(0.5, 64.0).unwrap_err()),
        "transient_points"
    );
}

#[test]
fn dc_sweep_point_budget() {
    for (start, stop, step) in [(0.0, 1.0, 1e-9), (0.0, 1.0, 1e-320), (0.0, 1e300, 1.0)] {
        let mut sim = sim_with(DIV, |_| {});
        let t0 = std::time::Instant::now();
        let e = sim
            .run_dc_sweep("V1", start, stop, step)
            .expect_err("budget");
        assert_eq!(resource(e), "dc_sweep_points");
        assert!(t0.elapsed().as_secs_f64() < 1.0);
    }
    // within budget: 0..1 step 0.1 -> 11 points
    let mut sim = sim_with(DIV, |_| {});
    let r = sim.run_dc_sweep("V1", 0.0, 1.0, 0.1).unwrap();
    assert_eq!(r.sweep_values.len(), 11);
    assert!((r.solutions[10][1] - 0.5).abs() < 1e-12);
}

#[test]
fn dc_sweep_that_cannot_advance_is_refused() {
    // ulp(1e17) = 16: `cur += 1.0` never changes `cur`; the former loop appended points forever
    let mut sim = sim_with(DIV, |_| {});
    let e = sim
        .run_dc_sweep("V1", 1e17, 1e17 + 100.0, 1.0)
        .expect_err("no progress");
    assert!(matches!(e, SimError::TopologyError(m) if m.contains("cannot advance")));
}

#[test]
fn dc_sweep_nonfinite_bounds_are_refused() {
    let mut sim = sim_with(DIV, |_| {});
    for (a, b, c) in [
        (0.0, f64::INFINITY, 1.0),
        (f64::NAN, 1.0, 1.0),
        (0.0, 1.0, f64::NAN),
    ] {
        assert!(matches!(
            sim.run_dc_sweep("V1", a, b, c),
            Err(SimError::TopologyError(_))
        ));
    }
}

#[test]
fn linear_overflow_is_a_loud_error_not_inf_output() {
    // v(1) = 1e308 V across 2 x 1 mohm: i(v1) = -5e310 A is not representable in f64
    let mut sim =
        Simulator::from_spice("ovf\nV1 1 0 1e308\nR1 1 2 1e-3\nR2 2 0 1e-3\n.op\n.end\n").unwrap();
    assert!(matches!(
        sim.run_dc_op(),
        Err(SimError::NonFiniteSolution { .. })
    ));
    // the same circuit at 1 V solves (control: the guard does not fire on finite results)
    let mut ok =
        Simulator::from_spice("ok\nV1 1 0 1\nR1 1 2 1e-3\nR2 2 0 1e-3\n.op\n.end\n").unwrap();
    let op = ok.run_dc_op().unwrap();
    assert!((op.solution[0] - 1.0).abs() < 1e-12 && (op.solution[1] - 0.5).abs() < 1e-12);
}

#[test]
fn open_circuit_resistor_literal_still_solves() {
    // control for the guard: 1e999 parses to +inf = open circuit, the finite answer v(2) = 5 V is unchanged
    let mut sim =
        Simulator::from_spice("open\nV1 1 0 5\nR1 1 2 1k\nR2 2 0 1e999\n.op\n.end\n").unwrap();
    let op = sim.run_dc_op().unwrap();
    assert!((op.solution[0] - 5.0).abs() < 1e-12 && (op.solution[1] - 5.0).abs() < 1e-12);
}
