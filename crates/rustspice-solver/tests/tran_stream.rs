//! Row streaming of `.tran` ([`Simulator::run_transient_spec_into`]): a sink receives exactly the rows the in-memory
//! result holds, bit for bit, with the row count announced up front (except with internal points), and a sink error
//! stops the analysis.

use rustspice_core::netlist::AnalysisCmd;
use rustspice_core::{SimError, SimResult};
use rustspice_solver::{DiscardRows, Simulator, TranResult, TranSink, TranSpec, TranStats};

/// Linear RC ladder driven by a periodic PULSE: interpolated rows, restarts at every corner and factor-cache hits.
const RC_PULSE: &str = "RC ladder, periodic pulse
V1 in 0 PULSE(0 1 0.2n 0.1n 0.1n 1n 2.5n)
R1 in a 1k
C1 a 0 0.2p
R2 a b 2k
C2 b 0 0.3p
R3 b out 1k
C3 out 0 0.1p
.tran 20p 10n
.end
";

/// Nonlinear: diode clamp with series resistance (an internal node) behind an RC, sine drive, `tstart` offset.
const DIODE_SINE: &str = "Diode clamp, sine drive
V1 in 0 SIN(0 2 500meg)
R1 in a 500
D1 a 0 DM
C1 a 0 1p
.model DM D(IS=1e-14 N=1.2 RS=5)
.tran 10p 6n 1n
.end
";

fn spec_of(sim: &Simulator) -> TranSpec {
    sim.netlist
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
        .expect("deck has a .tran card")
}

/// Records what the solver hands a sink.
#[derive(Default)]
struct Rec {
    begin: Vec<(Option<usize>, usize)>,
    t: Vec<f64>,
    x: Vec<Vec<f64>>,
}

impl TranSink for Rec {
    fn begin(&mut self, rows: Option<usize>, dim: usize) -> SimResult<()> {
        self.begin.push((rows, dim));
        Ok(())
    }
    fn row(&mut self, t: f64, x: &[f64]) -> SimResult<()> {
        self.t.push(t);
        self.x.push(x.to_vec());
        Ok(())
    }
}

fn collected(deck: &str, internal_points: bool) -> (TranResult, usize) {
    let mut sim = Simulator::from_spice(deck).unwrap();
    sim.transient_solver.options.internal_points = internal_points;
    let spec = spec_of(&sim);
    (
        sim.run_transient_spec(&spec).unwrap(),
        sim.topology.soa.mna_dim,
    )
}

fn streamed(deck: &str, internal_points: bool, sink: &mut dyn TranSink) -> TranStats {
    let mut sim = Simulator::from_spice(deck).unwrap();
    sim.transient_solver.options.internal_points = internal_points;
    let spec = spec_of(&sim);
    sim.run_transient_spec_into(&spec, sink).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn streamed_rows_are_the_collected_rows_bit_for_bit() {
    for deck in [RC_PULSE, DIODE_SINE] {
        for internal_points in [false, true] {
            let (res, dim) = collected(deck, internal_points);
            let mut rec = Rec::default();
            let st = streamed(deck, internal_points, &mut rec);
            let what = format!(
                "{} internal_points={internal_points}",
                deck.lines().next().unwrap()
            );
            assert!(res.time_points.len() > 100, "{what}");
            assert_eq!(st.rows, res.time_points.len(), "{what}");
            assert_eq!(bits(&rec.t), bits(&res.time_points), "{what}");
            assert_eq!(rec.x.len(), res.waveforms.len(), "{what}");
            for (k, (a, b)) in rec.x.iter().zip(&res.waveforms).enumerate() {
                assert_eq!(bits(a), bits(b), "{what}: row {k}");
            }
            assert_eq!(
                (
                    st.total_nr_iterations,
                    st.lu_factorizations,
                    st.accepted_steps,
                    st.rejected_steps
                ),
                (
                    res.total_nr_iterations,
                    res.lu_factorizations,
                    res.accepted_steps,
                    res.rejected_steps
                ),
                "{what}"
            );
            // the row count is announced exactly when it is known in advance
            let announced = (!internal_points).then_some(st.rows);
            assert_eq!(rec.begin, vec![(announced, dim)], "{what}");
        }
    }
}

#[test]
fn internal_points_add_rows_between_the_grid_rows() {
    let (plain, _) = collected(RC_PULSE, false);
    let mut rec = Rec::default();
    let st = streamed(RC_PULSE, true, &mut rec);
    assert!(st.rows > plain.time_points.len());
    // the grid rows are all there, with the same values
    for (t, x) in plain.time_points.iter().zip(&plain.waveforms) {
        let k = rec.t.iter().position(|u| u == t).expect("grid row present");
        assert_eq!(bits(&rec.x[k]), bits(x));
    }
}

#[test]
fn a_discarding_sink_runs_the_same_analysis() {
    let (res, _) = collected(RC_PULSE, false);
    let st = streamed(RC_PULSE, false, &mut DiscardRows);
    assert_eq!(st.rows, res.time_points.len());
    assert_eq!(st.lu_factorizations, res.lu_factorizations);
    assert_eq!(st.accepted_steps, res.accepted_steps);
}

#[test]
fn a_sink_error_stops_the_analysis() {
    struct FailAt(usize, usize);
    impl TranSink for FailAt {
        fn row(&mut self, _t: f64, _x: &[f64]) -> SimResult<()> {
            self.1 += 1;
            if self.1 == self.0 {
                return Err(SimError::BackendError {
                    backend: "sink".to_string(),
                    message: "disk full".to_string(),
                });
            }
            Ok(())
        }
    }
    let mut sim = Simulator::from_spice(RC_PULSE).unwrap();
    let spec = spec_of(&sim);
    let mut sink = FailAt(10, 0);
    let err = sim.run_transient_spec_into(&spec, &mut sink).unwrap_err();
    assert!(err.to_string().contains("disk full"), "{err}");
    assert_eq!(sink.1, 10, "no row after the failing one");
    // the simulator stays usable: the sources carry their card values again
    let again = sim.run_transient_spec(&spec).unwrap();
    let (fresh, _) = collected(RC_PULSE, false);
    assert_eq!(bits(&again.time_points), bits(&fresh.time_points));
}
