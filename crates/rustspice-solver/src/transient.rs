//! Time-domain Transient Integrator: trapezoidal rule (default), `BDF-1` and variable-step `BDF-2`, with
//! local-truncation-error (LTE) step control, waveform breakpoints, `.ic` / `uic` start points and
//! constant-$h$ LU reuse for linear circuits.
//!
//! # Time control
//!
//! * Output rows sit on the nominal grid `k * tstep` (plus `tstop`, and `tstart` when it is off-grid). A
//!   nonlinear (or memoryless) circuit lands on them, stretching or shortening a step within `[0.5, 1.25] h`;
//!   a linear circuit keeps its step (and its LU factors) and interpolates the rows between accepted steps
//!   (quadratic through the last three accepted points of the current segment, linear right after a
//!   breakpoint). A row that coincides with an accepted step is that step.
//! * Internal steps are bounded by `hmax = min(tstep, tmax, (tstop - tstart) / 50)` and chosen from a
//!   divided-difference estimate of the LTE of every capacitor voltage and inductor current
//!   (`h^3 |y'''| / 12` for the trapezoid). A step whose estimate exceeds the tolerance is rejected and
//!   repeated with a smaller step.
//! * Every slope discontinuity of an independent source (PULSE corners, delayed SIN start) is a breakpoint:
//!   the integrator lands on it and restarts with two backward-Euler steps of `h1` and one BDF-2 step of
//!   `phi h1` before it resumes the trapezoid at `2 h1` (all four share one companion factor, so a linear circuit
//!   factors once). The L-stable restart damps the stiff modes excited by the corner, which the trapezoid would
//!   otherwise carry on as a sign-alternating ("ringing") error, and the BDF-2 step hands the trapezoid a
//!   second-order accurate capacitor current / inductor voltage.

use rustspice_core::{
    netlist::SourceWaveform, CompiledTopology, ComputeBackend, DeviceSoaBuffer, ResourceLimits,
    SimError, SimResult,
};
use serde::{Deserialize, Serialize};

use crate::linear::{AdaptiveSparseSolver, FactorSnapshot};
use crate::nonlinear::NonlinearSolver;

/// Numerical integration scheme for transient differential-algebraic equations (DAEs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrationMethod {
    /// First-order A-stable Backward Euler (BDF-1).
    Bdf1,
    /// Second-order A-stable Trapezoidal rule (default in SPICE).
    Trapezoidal,
    /// Second-order L-stable Gear-2 (BDF-2).
    Bdf2,
}

impl IntegrationMethod {
    /// Order of accuracy `p` (the LTE is `O(h^(p+1))`).
    #[inline]
    fn order(self) -> i32 {
        match self {
            Self::Bdf1 => 1,
            Self::Trapezoidal | Self::Bdf2 => 2,
        }
    }
}

/// Configuration options for transient simulation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TranOptions {
    pub method: IntegrationMethod,
    pub use_predictor: bool,
    /// Relative LTE tolerance (SPICE `reltol`).
    pub reltol: f64,
    /// Absolute LTE tolerance of capacitor voltages in volts (SPICE `vntol`).
    pub vntol: f64,
    /// Smallest absolute LTE tolerance of inductor currents in amperes (SPICE `abstol`).
    pub abstol: f64,
    /// Factor by which the LTE estimate may exceed the tolerance (SPICE `trtol`).
    pub trtol: f64,
}

impl Default for TranOptions {
    fn default() -> Self {
        Self {
            method: IntegrationMethod::Trapezoidal,
            use_predictor: true,
            reltol: 1e-3,
            vntol: 1e-6,
            abstol: 1e-12,
            trtol: 7.0,
        }
    }
}

/// What a `.tran` card asks for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranSpec {
    /// Output step: rows are written at `k * tstep`.
    pub tstep: f64,
    pub tstop: f64,
    /// First output row (rows before it are integrated but not stored).
    pub tstart: f64,
    /// Upper bound of the internal step (`None`: `min(tstep, (tstop - tstart) / 50)`).
    pub tmax: Option<f64>,
    /// Skip the operating point and start from the element `ic=` / `.ic` values (capacitors and inductors
    /// without one start at zero).
    pub uic: bool,
}

impl TranSpec {
    pub fn new(tstep: f64, tstop: f64) -> Self {
        Self {
            tstep,
            tstop,
            tstart: 0.0,
            tmax: None,
            uic: false,
        }
    }
}

/// Time-domain waveform output from a transient simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranResult {
    pub time_points: Vec<f64>,
    pub waveforms: Vec<Vec<f64>>,
    pub total_nr_iterations: usize,
    pub lu_factorizations: usize,
    /// Internal integration steps that were accepted.
    #[serde(default)]
    pub accepted_steps: usize,
    /// Internal steps repeated with a smaller step (LTE above tolerance or Newton failure).
    #[serde(default)]
    pub rejected_steps: usize,
}

/// High-speed Transient Integrator.
pub struct TransientSolver {
    pub options: TranOptions,
    /// Work / memory budgets enforced by `solve_transient`.
    pub limits: ResourceLimits,
    rhs_scratch: Vec<f64>,
    /// Capacitor voltages / inductor currents two accepted steps back (`y_{n-2}`), for the LTE estimate.
    cap_v_prev3: Vec<f64>,
    ind_i_prev3: Vec<f64>,
}

/// Fraction of `hmax` used for the first step after a breakpoint (and after `t = 0`): a power of two, so that
/// doubling the step reaches `hmax` exactly.
const RESTART_FRACTION: f64 = 0.125;
/// Ratio of the BDF-2 step to the backward-Euler step that precedes it after a restart: with `w = phi` (golden
/// ratio, `w^2 = w + 1`) the BDF-2 companion factor `(1 + 2w) / ((1 + w) h_2)` equals the backward-Euler one
/// `1 / h_1`, and the trapezoid step `2 h_1` that follows has it too, so a linear circuit factors its matrix once
/// for all three steps.
const BDF2_STARTUP_RATIO: f64 = 1.618_033_988_749_895;
/// Largest growth of the step from one accepted step to the next.
const MAX_GROWTH: f64 = 2.0;
/// Smallest reduction after a rejected step (a larger cut would waste the information of the estimate).
const MIN_SHRINK: f64 = 0.2;
/// Safety factor applied to the step the LTE estimate allows.
const SAFETY: f64 = 0.9;
/// Smallest growth worth taking: a step allowed to grow by less is kept as it is.
const MIN_GROWTH: f64 = 1.2;
/// Newton failures tolerated on one step (each one divides the step by 4) before the analysis fails.
const MAX_NEWTON_RETRIES: usize = 24;

/// The output rows of a `.tran` card: `tstart` (when off-grid), then `k * tstep` from `tstart` on, then
/// `tstop`. A tail shorter than `tol_q` steps is merged into the last regular interval, whose row is stamped
/// `tstop` (an accumulated `t += h` used to drift and append sliver steps).
struct OutputGrid {
    tstep: f64,
    tstop: f64,
    n_steps: usize,
    next_k: usize,
    pending_tstart: Option<f64>,
}

impl OutputGrid {
    fn new(tstep: f64, tstop: f64, tstart: f64) -> Self {
        let q = tstop / tstep;
        let tol_q = 1e-9 + 4.0 * f64::EPSILON * q;
        let n_full = (q + tol_q).floor() as usize;
        let tail = tstop - (n_full as f64) * tstep;
        let has_tail = tail > tol_q * tstep;
        let n_steps = n_full.saturating_add(usize::from(has_tail));
        let mut grid = Self {
            tstep,
            tstop,
            n_steps,
            next_k: 0,
            pending_tstart: None,
        };
        if tstart > 0.0 {
            let k0 = ((tstart / tstep) - tol_q).ceil().max(0.0) as usize;
            grid.next_k = k0.min(n_steps);
            if grid.time(grid.next_k) > tstart * (1.0 + 1e-12) {
                grid.pending_tstart = Some(tstart);
            }
        }
        grid
    }

    #[inline]
    fn time(&self, k: usize) -> f64 {
        if k >= self.n_steps {
            self.tstop
        } else {
            (k as f64) * self.tstep
        }
    }

    /// Number of rows still to be written.
    fn remaining(&self) -> usize {
        usize::from(self.pending_tstart.is_some()) + (self.n_steps + 1).saturating_sub(self.next_k)
    }

    fn peek(&self) -> Option<f64> {
        if let Some(t) = self.pending_tstart {
            return Some(t);
        }
        (self.next_k <= self.n_steps).then(|| self.time(self.next_k))
    }

    fn advance(&mut self) {
        if self.pending_tstart.take().is_none() {
            self.next_k += 1;
        }
    }
}

/// Step toward an instant `left` ahead with a proposed step `h_prop`: the instant in one step when it is at most
/// `stretch` times the proposed step away, half of it when it is less than `2 stretch` steps away (two equal
/// steps, no sliver), else the proposed step. Returns the step and whether it lands. A retry after a rejected
/// step plans with `stretch = 1`, so it never comes back to the rejected length.
#[inline]
/// Largest step `hmax / 2^k` not above `h`: a linear circuit only takes these, so the companion matrices it meets
/// are few and come back at every restart (see [`FactorCache`]).
fn quantize_step(h: f64, hmax: f64) -> f64 {
    if h >= hmax {
        return hmax;
    }
    let k = (hmax / h).log2().ceil().min(1000.0);
    hmax * 0.5_f64.powi(k as i32)
}

fn plan_step(left: f64, h_prop: f64, stretch: f64) -> (f64, bool) {
    if h_prop * stretch >= left {
        (left, true)
    } else if 2.0 * stretch * h_prop > left {
        (0.5 * left, false)
    } else {
        (h_prop, false)
    }
}

/// End of a step of length `h` from `t`: `stop` when the step lands on it; the next output row when the step
/// ends within `1e-6 h` of it (the time is snapped to the row so that rounding does not accumulate along a long
/// run of equal steps; the companion models keep `h`, and the shift is far below the truncation error).
#[inline]
fn step_end(t: f64, h: f64, lands: bool, stop: f64, next_row: Option<f64>) -> f64 {
    if lands {
        return stop;
    }
    let end = t + h;
    match next_row {
        Some(row) if (row - end).abs() <= 1e-6 * h => row,
        _ => end,
    }
}

/// Value at `tau` of the polynomial through the points `(t_k, y_k)`: quadratic, or linear when the third time is
/// `NaN` (Lagrange form, evaluated per unknown).
fn interpolate_row(tau: f64, pts: [(f64, &Vec<f64>); 3]) -> Vec<f64> {
    let [(ta, ya), (tb, yb), (tc, yc)] = pts;
    if tc.is_nan() {
        let wa = (tau - tb) / (ta - tb);
        return ya
            .iter()
            .zip(yb)
            .map(|(a, b)| wa * a + (1.0 - wa) * b)
            .collect();
    }
    let la = (tau - tb) * (tau - tc) / ((ta - tb) * (ta - tc));
    let lb = (tau - ta) * (tau - tc) / ((tb - ta) * (tb - tc));
    let lc = (tau - ta) * (tau - tb) / ((tc - ta) * (tc - tb));
    ya.iter()
        .zip(yb)
        .zip(yc)
        .map(|((a, b), c)| la * a + lb * b + lc * c)
        .collect()
}

/// Bounds of [`FactorCache`]: entries whose size is known (unrolled / dense factors) and their total bytes, and
/// entries of `faer` factors (each as large as the live factors, so memory stays within that many extra copies).
const FACTOR_CACHE_ENTRIES: usize = 8;
const FACTOR_CACHE_BYTES: usize = 64 << 20;
const FACTOR_CACHE_FAER_ENTRIES: usize = 4;

/// LU factors of a linear circuit for the companion scales the integration revisits: every restart at a source
/// corner climbs the same ladder (backward Euler at `h1`, trapezoid at `2 h1`, `4 h1`, ... up to `hmax`) and lands on
/// the next corner with the same remainder step in every period of a periodic source; refactoring these dominated
/// large linear transients. Least-recently-used eviction within the bounds above.
#[derive(Default)]
struct FactorCache {
    /// Oldest use first.
    entries: Vec<(f64, FactorSnapshot)>,
}

impl FactorCache {
    fn same_scale(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b
    }

    /// Restores the factors of companion scale `g` into `solver` when they are cached.
    fn restore(&mut self, solver: &mut AdaptiveSparseSolver, g: f64) -> bool {
        let Some(k) = self
            .entries
            .iter()
            .position(|(s, _)| Self::same_scale(*s, g))
        else {
            return false;
        };
        let e = self.entries.remove(k);
        let ok = solver.restore_factors(&e.1);
        if ok {
            self.entries.push(e);
        }
        ok
    }

    fn insert(&mut self, g: f64, snap: FactorSnapshot) {
        self.entries.retain(|(s, _)| !Self::same_scale(*s, g));
        let (max_entries, max_bytes) = match snap.known_bytes() {
            Some(_) => (FACTOR_CACHE_ENTRIES, FACTOR_CACHE_BYTES),
            None => (FACTOR_CACHE_FAER_ENTRIES, usize::MAX),
        };
        let bytes = |e: &[(f64, FactorSnapshot)]| -> usize {
            e.iter().map(|(_, f)| f.known_bytes().unwrap_or(0)).sum()
        };
        let need = snap.known_bytes().unwrap_or(0);
        if need > max_bytes {
            return;
        }
        while !self.entries.is_empty()
            && (self.entries.len() >= max_entries || bytes(&self.entries) + need > max_bytes)
        {
            self.entries.remove(0);
        }
        self.entries.push((g, snap));
    }
}

/// Where the integration restarts its order (BE twice, then BDF-2, then the configured method).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// First backward-Euler step.
    Startup,
    /// Second backward-Euler step (same `h`): damps what is left of the stiff modes the corner excited, so that
    /// the BDF-2 step does not see the pre-corner amplitude in its history.
    Damping,
    /// BDF-2 step: second-order capacitor currents / inductor voltages for the trapezoid.
    Second,
    Running,
}

impl TransientSolver {
    pub fn new(options: TranOptions) -> Self {
        Self {
            options,
            limits: ResourceLimits::default(),
            rhs_scratch: Vec::new(),
            cap_v_prev3: Vec::new(),
            ind_i_prev3: Vec::new(),
        }
    }

    /// Runs a transient analysis from `t = 0.0` to `tstop` with output step `tstep`, starting from the
    /// operating point (with `initial_conditions` forced while it is solved).
    #[allow(clippy::too_many_arguments)]
    pub fn solve_transient(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        nonlinear_solver: &mut NonlinearSolver,
        tstep: f64,
        tstop: f64,
        initial_conditions: &rustc_hash::FxHashMap<usize, f64>,
    ) -> SimResult<TranResult> {
        self.solve_transient_spec(
            topo,
            backend,
            linear_solver,
            nonlinear_solver,
            &TranSpec::new(tstep, tstop),
            initial_conditions,
        )
    }

    /// Runs the transient analysis a `.tran` card describes (see [`TranSpec`]).
    pub fn solve_transient_spec(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        nonlinear_solver: &mut NonlinearSolver,
        spec: &TranSpec,
        initial_conditions: &rustc_hash::FxHashMap<usize, f64>,
    ) -> SimResult<TranResult> {
        // SPICE timing defaults (PULSE rise / fall = tstep, SIN frequency = 1/tstop, ...) depend on this `.tran`
        // card: resolve them on the source tables for this run and put the card values back afterwards
        let soa = &mut topo.soa;
        let timed = soa
            .vsrc_waveforms
            .iter()
            .chain(soa.isrc_waveforms.iter())
            .any(|w| !matches!(w, SourceWaveform::Dc(_)));
        let saved = timed.then(|| (soa.vsrc_waveforms.clone(), soa.isrc_waveforms.clone()));
        if timed {
            for w in soa
                .vsrc_waveforms
                .iter_mut()
                .chain(soa.isrc_waveforms.iter_mut())
            {
                w.resolve_defaults(spec.tstep, spec.tstop);
            }
        }
        let result = self.run_spec(
            topo,
            backend,
            linear_solver,
            nonlinear_solver,
            spec,
            initial_conditions,
        );
        if let Some((v, i)) = saved {
            topo.soa.vsrc_waveforms = v;
            topo.soa.isrc_waveforms = i;
        }
        result
    }

    fn run_spec(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        nonlinear_solver: &mut NonlinearSolver,
        spec: &TranSpec,
        initial_conditions: &rustc_hash::FxHashMap<usize, f64>,
    ) -> SimResult<TranResult> {
        let TranSpec {
            tstep,
            tstop,
            tstart,
            tmax,
            uic,
        } = *spec;
        if !(tstep.is_finite() && tstop.is_finite()) {
            return Err(SimError::TransientFailure {
                time: 0.0,
                reason: format!(
                    "tstep and tstop must be finite (tstep = {tstep}, tstop = {tstop})"
                ),
            });
        }
        if tstep <= 0.0 || tstop <= 0.0 {
            return Err(SimError::TransientFailure {
                time: 0.0,
                reason: "tstep and tstop must be positive".to_string(),
            });
        }
        let tstart = if tstart.is_finite() && tstart > 0.0 {
            tstart
        } else {
            0.0
        };
        if tstart >= tstop {
            return Err(SimError::TransientFailure {
                time: 0.0,
                reason: format!("tstart ({tstart:e}) must be smaller than tstop ({tstop:e})"),
            });
        }

        let dim = topo.soa.mna_dim;
        self.rhs_scratch.resize(dim, 0.0);

        // Budget check BEFORE any work or allocation: `tstop / tstep` can be 1e12 or 1e300.
        let est_points = ((tstop - tstart) / tstep).ceil() + 2.0;
        self.limits
            .check_points("transient_points", est_points, dim)?;
        let point_cap = self.limits.point_cap(dim);
        // Internal steps are not stored, but they are work: bound them as well (Rule 9).
        let step_budget = (64.0 * est_points).max(4_000_000.0) as u64;

        let mut hmax = tstep.min((tstop - tstart) / 50.0);
        if let Some(tm) = tmax.filter(|v| v.is_finite() && *v > 0.0) {
            hmax = hmax.min(tm);
        }
        // Breakpoints closer than this are one instant; steps shorter than `hmin` are never forced.
        let teps = 1e-9 * hmax;
        let hmin = 1e-12 * hmax;

        // 1. Start point at t = 0
        topo.soa.update_sources_at_time(Some(0.0));
        let (mut x, start_iters) = if uic {
            self.uic_start(
                topo,
                backend,
                linear_solver,
                nonlinear_solver,
                hmax,
                initial_conditions,
            )?
        } else {
            self.op_start(
                topo,
                backend,
                linear_solver,
                nonlinear_solver,
                initial_conditions,
            )?
        };
        self.cap_v_prev3.clear();
        self.cap_v_prev3.extend_from_slice(&topo.soa.cap_v_prev);
        self.ind_i_prev3.clear();
        self.ind_i_prev3.extend_from_slice(&topo.soa.ind_i_prev);

        let mut grid = OutputGrid::new(tstep, tstop, tstart);
        let est_rows = grid.remaining();
        let mut time_points = Vec::with_capacity(est_rows);
        let mut waveforms = Vec::with_capacity(est_rows);
        if grid.peek() == Some(0.0) {
            time_points.push(0.0);
            waveforms.push(x.clone());
            grid.advance();
        }

        topo.soa.is_transient = true;
        linear_solver.invalidate_factorization();

        let method_cfg = self.options.method;
        let mut h_next = RESTART_FRACTION * hmax;
        // Refresh static-pivot symbolic LU with the transient companion conductances stamped on the diagonal so
        // capacitor/inductor meshes and ladders select diagonally dominant pivots rather than the open-cap /
        // short-inductor DC operating-point pivots.
        if !topo.soa.cap_names.is_empty() || !topo.soa.ind_names.is_empty() {
            Self::update_companion(topo, IntegrationMethod::Bdf1, h_next, 0.0);
            backend.evaluate_devices(&mut topo.soa, &x)?;
            backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs_scratch)?;
            linear_solver.refresh_for_transient(&topo.jacobian);
            linear_solver.invalidate_factorization();
        }

        let is_linear = topo.soa.is_linear();
        let has_reactive = !topo.soa.cap_names.is_empty() || !topo.soa.ind_names.is_empty();
        // a linear circuit keeps its steps on the `hmax / 2^k` ladder
        let quantized = is_linear && has_reactive;
        let land_rows = !is_linear || !has_reactive;
        let mut x_acc = x.clone();
        let mut x_acc_prev = x.clone();
        let mut total_nr_iterations = start_iters;
        let mut lu_factorizations = 1usize;
        let mut accepted_steps = 0usize;
        let mut rejected_steps = 0usize;
        // Accepted times t_n, t_{n-1}, t_{n-2} and how many of them lie after the last restart.
        let mut t_hist = [0.0_f64; 3];
        let mut n_hist = 1usize;
        let mut stage = Stage::Startup;
        // `g_scale` of the factorization a linear circuit may reuse (`None`: no reusable factorization).
        let mut factored_scale: Option<f64> = None;
        let mut factor_cache = FactorCache::default();
        // a source jumped at the last restart point: the rows inside the first step after it take that step's
        // (post-jump) solution
        let mut jump_restart = false;
        let mut next_src_bp = Self::next_source_breakpoint(&topo.soa, 0.0, teps, tstop);
        if let Some(bp) = next_src_bp {
            h_next = h_next.min(0.25 * bp).max(hmin);
            if quantized {
                h_next = quantize_step(h_next, hmax).max(hmin);
            }
        }

        let mut t = 0.0_f64;
        while grid.peek().is_some() {
            if time_points.len() as u64 >= point_cap {
                return Err(SimError::ResourceLimit {
                    resource: "transient_points",
                    requested: format!(
                        "more than {point_cap} time points (stopped at t = {t:.6e} s)"
                    ),
                    limit: format!("{point_cap} points (max_points / max_result_bytes)"),
                    hint: "enlarge the step, shorten the analysis, or raise --max-points / --max-result-mib"
                        .to_string(),
                });
            }
            if (accepted_steps + rejected_steps) as u64 >= step_budget {
                return Err(SimError::ResourceLimit {
                    resource: "transient_steps",
                    requested: format!(
                        "more than {step_budget} internal steps (stopped at t = {t:.6e} s)"
                    ),
                    limit: format!("{step_budget} steps (64 per output row, at least 4e6)"),
                    hint: "the circuit needs far smaller steps than tstep; enlarge tstep or shorten the analysis"
                        .to_string(),
                });
            }

            // Next instant the step must land on: the next source breakpoint, else `tstop`.
            let (stop, stop_is_bp) = match next_src_bp {
                Some(bp) if bp < tstop - teps => (bp, true),
                _ => (tstop, false),
            };
            let left = stop - t;
            if left <= teps {
                // an instant that coincides with the current time: write its rows (all remaining ones at
                // `tstop`) and move on to the next breakpoint
                while let Some(t_out) = grid.peek() {
                    if stop_is_bp && t_out > t + teps {
                        break;
                    }
                    time_points.push(t_out);
                    waveforms.push(x_acc.clone());
                    grid.advance();
                }
                if stop_is_bp {
                    next_src_bp = Self::next_source_breakpoint(&topo.soa, t, teps, tstop);
                }
                continue;
            }

            let step_method = match (stage, method_cfg) {
                (_, IntegrationMethod::Bdf1) | (Stage::Startup | Stage::Damping, _) => {
                    IntegrationMethod::Bdf1
                }
                (Stage::Second, _) => IntegrationMethod::Bdf2,
                (Stage::Running, m) => m,
            };
            let h_prev = t_hist[0] - t_hist[1];
            let mut h_prop = h_next.min(hmax).max(hmin);
            if step_method == IntegrationMethod::Bdf2 && n_hist >= 2 && h_prev > 0.0 {
                // variable-step BDF-2 is zero-stable for h / h_prev < 1 + sqrt(2)
                h_prop = h_prop.min(2.0 * h_prev);
            }
            // The instant this step aims at: the breakpoint / `tstop`, or, for a nonlinear (or memoryless)
            // circuit, the next output row before it (its solution has kinks that no interpolation follows, and
            // it factors its matrix at every Newton iteration anyway). A linear circuit keeps its step and its LU
            // factors and interpolates the rows.
            let (stop, stop_is_bp) = match grid.peek() {
                Some(row) if land_rows && row < stop - teps => (row, false),
                _ => (stop, stop_is_bp),
            };
            let (mut h, mut lands) = plan_step(stop - t, h_prop, 1.25);
            let mut newton_retries = 0usize;
            let (r, order) = loop {
                let t_new = step_end(t, h, lands, stop, grid.peek());
                // a step that lands on a breakpoint sees the sources' left limits there (a jump belongs to the
                // next step)
                let t_src = if lands && stop_is_bp {
                    t_new - teps
                } else {
                    t_new
                };
                topo.soa.update_sources_at_time(Some(t_src));
                let g_scale = Self::update_companion(topo, step_method, h, h_prev);

                // Steps that land on the grid differ from `tstep` by a few ulps of `t` (2e-12 relative at 1e4
                // rows): a companion conductance 1e-9 off the factored one changes the solution far below the
                // Newton tolerance, so the factors are reused.
                let solved = if is_linear
                    && factored_scale.is_some_and(|g| (g - g_scale).abs() <= 1e-9 * g_scale)
                    && linear_solver.has_valid_factorization()
                {
                    self.solve_linear_cached(topo, linear_solver, &mut x)
                        .map(|()| 1usize)
                } else if is_linear && factor_cache.restore(linear_solver, g_scale) {
                    factored_scale = Some(g_scale);
                    self.solve_linear_cached(topo, linear_solver, &mut x)
                        .map(|()| 1usize)
                } else {
                    // initial guess for Newton: extrapolation of the last two accepted points
                    if self.options.use_predictor
                        && !is_linear
                        && stage == Stage::Running
                        && n_hist >= 2
                        && h_prev > 0.0
                    {
                        let ratio = (h / h_prev).min(1.0);
                        for i in 0..dim {
                            x[i] = x_acc[i] + ratio * (x_acc[i] - x_acc_prev[i]);
                        }
                    } else {
                        x.copy_from_slice(&x_acc);
                    }
                    // Junction limiting (pnjlim) must be relative to the last converged step, not to the
                    // extrapolated guess: a predictor that overshoots a switching edge would otherwise start
                    // Newton unlimited, deep in the exponential.
                    NonlinearSolver::sync_device_voltages(topo, &x_acc);
                    let res = nonlinear_solver
                        .solve_in_place(topo, backend, linear_solver, &mut x)
                        .map(|(iters, _)| iters);
                    if is_linear {
                        factored_scale = res.is_ok().then_some(g_scale);
                        if res.is_ok() {
                            if let Some(snap) = linear_solver.snapshot_factors() {
                                factor_cache.insert(g_scale, snap);
                            }
                        }
                    }
                    if let Ok(iters) = res {
                        lu_factorizations += iters;
                    }
                    res
                };

                match solved {
                    Ok(iters) => {
                        total_nr_iterations += iters;
                        if let Err(SimError::NonFiniteSolution { unknown }) =
                            NonlinearSolver::ensure_finite(&x)
                        {
                            return Err(SimError::TransientFailure {
                                time: t_new,
                                reason: format!(
                                    "non-finite value (overflow or NaN) in MNA unknown {unknown}"
                                ),
                            });
                        }
                    }
                    Err(e) => {
                        newton_retries += 1;
                        rejected_steps += 1;
                        if newton_retries > MAX_NEWTON_RETRIES || h * 0.25 < hmin {
                            return Err(SimError::TransientFailure {
                                time: t_new,
                                reason: format!("Step convergence failed after retries: {e}"),
                            });
                        }
                        (h, lands) = plan_step(stop - t, h * 0.25, 1.0);
                        x.copy_from_slice(&x_acc);
                        NonlinearSolver::sync_device_voltages(topo, &x_acc);
                        continue;
                    }
                }

                // LTE estimate (none during the restart: too little history on this side of the corner)
                let order = step_method.order();
                let r = if !has_reactive || stage != Stage::Running {
                    0.0
                } else {
                    self.lte_ratio(topo, &x, step_method, h, t_new, &t_hist, n_hist)
                };
                if r > 1.0 && h > hmin * 4.0 {
                    rejected_steps += 1;
                    let fit = r.powf(-1.0 / f64::from(order + 1));
                    let shrink = (SAFETY * fit).clamp(MIN_SHRINK, SAFETY);
                    let mut h_try = (h * shrink).max(hmin);
                    if quantized {
                        h_try = quantize_step(h_try, hmax).max(hmin);
                    }
                    (h, lands) = plan_step(stop - t, h_try, 1.0);
                    x.copy_from_slice(&x_acc);
                    NonlinearSolver::sync_device_voltages(topo, &x_acc);
                    continue;
                }
                break (r, order);
            };

            // 2. Accept the step, writing the rows it passes
            let t_new = step_end(t, h, lands, stop, grid.peek());
            while let Some(t_out) = grid.peek() {
                if t_out > t_new + teps || time_points.len() as u64 >= point_cap {
                    break;
                }
                let row =
                    if (t_out - t_new).abs() <= teps || (jump_restart && stage == Stage::Startup) {
                        x.clone()
                    } else if n_hist >= 2 {
                        interpolate_row(t_out, [(t_new, &x), (t, &x_acc), (t_hist[1], &x_acc_prev)])
                    } else {
                        interpolate_row(t_out, [(t_new, &x), (t, &x_acc), (f64::NAN, &x_acc)])
                    };
                time_points.push(t_out);
                waveforms.push(row);
                grid.advance();
            }
            self.cap_v_prev3.copy_from_slice(&topo.soa.cap_v_prev2);
            self.ind_i_prev3.copy_from_slice(&topo.soa.ind_i_prev2);
            Self::commit_reactive_history(topo, &x);
            x_acc_prev.copy_from_slice(&x_acc);
            x_acc.copy_from_slice(&x);
            t_hist = [t_new, t_hist[0], t_hist[1]];
            n_hist = (n_hist + 1).min(4);
            accepted_steps += 1;
            t = t_new;

            // next step: what the estimate allows, never more than twice the step that was planned
            let grow = if r > (SAFETY / MAX_GROWTH).powi(order + 1) {
                (SAFETY * r.powf(-1.0 / f64::from(order + 1))).clamp(MIN_SHRINK, MAX_GROWTH)
            } else {
                MAX_GROWTH
            };
            // small changes are not worth a new factorization: a linear circuit's step only doubles (when the
            // estimate allows it) or stays, so it stays on the `hmax / 2^k` ladder
            let grow = if quantized {
                if grow >= 2.0 {
                    2.0
                } else {
                    1.0
                }
            } else if (1.0..MIN_GROWTH).contains(&grow) {
                1.0
            } else {
                grow
            };
            h_next = match stage {
                // the BDF-2 and first trapezoid steps after a restart share the backward-Euler companion factor
                Stage::Startup => h,
                Stage::Damping => h * BDF2_STARTUP_RATIO,
                Stage::Second => h * (2.0 / BDF2_STARTUP_RATIO),
                _ => h * grow,
            };
            stage = match stage {
                Stage::Startup => Stage::Damping,
                Stage::Damping => Stage::Second,
                Stage::Second | Stage::Running => Stage::Running,
            };

            // the first step after the restart is done
            if stage == Stage::Damping {
                jump_restart = false;
            }
            if lands && stop_is_bp {
                jump_restart = Self::sources_jump_at(&topo.soa, t, teps);
                // restart the order at the corner: its left history says nothing about the right side
                let gap = Self::next_source_breakpoint(&topo.soa, t, teps, tstop);
                let mut h1 = (RESTART_FRACTION * hmax).min(h_next);
                if let Some(g) = gap {
                    h1 = h1.min(0.25 * (g - t));
                }
                if quantized {
                    h1 = quantize_step(h1, hmax);
                }
                h_next = h1.max(hmin);
                next_src_bp = gap;
                stage = Stage::Startup;
                n_hist = 1;
            }
        }

        topo.soa.is_transient = false;
        Ok(TranResult {
            time_points,
            waveforms,
            total_nr_iterations,
            lu_factorizations,
            accepted_steps,
            rejected_steps,
        })
    }

    /// Operating-point start: the DC solution at `t = 0` with every `.ic` node clamped to its value while it is
    /// solved. Capacitors start at the solved voltage with zero current (the first step after `t = 0` is
    /// backward Euler, which does not use the current).
    fn op_start(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        nonlinear_solver: &mut NonlinearSolver,
        initial_conditions: &rustc_hash::FxHashMap<usize, f64>,
    ) -> SimResult<(Vec<f64>, usize)> {
        topo.soa.is_transient = false;
        NonlinearSolver::check_dc_solvable_with_ic(&topo.soa, Some(initial_conditions))?;
        topo.soa.set_ic_clamps(true);
        // Initial solution: Newton, then gmin stepping, then source stepping (as for `.op`)
        let op = nonlinear_solver.solve_op_at_current_sources(topo, backend, linear_solver);
        topo.soa.set_ic_clamps(false);
        let op = op?;
        let x = op.solution;
        for i in 0..topo.soa.cap_names.len() {
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_neg[i]);
            let vc0 = vp - vn;
            topo.soa.cap_v_prev[i] = vc0;
            topo.soa.cap_v_prev2[i] = vc0;
            topo.soa.cap_i_prev[i] = 0.0;
        }
        for i in 0..topo.soa.ind_names.len() {
            let il0 = x[topo.soa.ind_branch_idx[i]];
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_neg[i]);
            topo.soa.ind_i_prev[i] = il0;
            topo.soa.ind_i_prev2[i] = il0;
            topo.soa.ind_v_prev[i] = vp - vn;
        }
        Ok((x, op.iterations))
    }

    /// `uic` start: no operating point. Every capacitor starts at its `ic=` voltage (else the difference of the
    /// `.ic` values of its nodes, an unlisted node counting as 0 V) and every inductor at its `ic=` current
    /// (else 0 A). The `t = 0` row is the circuit solved with those states imposed (capacitors as stiff
    /// voltage sources, inductors as stiff current sources: the companion models of a backward-Euler step of
    /// `1e-9 hmax`), so node voltages and branch currents are consistent with them.
    fn uic_start(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        nonlinear_solver: &mut NonlinearSolver,
        hmax: f64,
        initial_conditions: &rustc_hash::FxHashMap<usize, f64>,
    ) -> SimResult<(Vec<f64>, usize)> {
        let dim = topo.soa.mna_dim;
        let node_ic = |node: usize| -> f64 {
            if node == 0 {
                0.0
            } else {
                initial_conditions.get(&node).copied().unwrap_or(0.0)
            }
        };
        let h0 = 1e-9 * hmax;
        topo.soa.is_transient = true;
        topo.soa.transient_g_scale = 1.0 / h0;
        let mut vc0 = vec![0.0_f64; topo.soa.cap_names.len()];
        for (i, v0) in vc0.iter_mut().enumerate() {
            let ic = topo.soa.cap_ic[i];
            *v0 = if ic.is_nan() {
                node_ic(topo.soa.cap_n_pos[i]) - node_ic(topo.soa.cap_n_neg[i])
            } else {
                ic
            };
            let g = topo.soa.cap_capacitance[i] / h0;
            topo.soa.cap_geq[i] = g;
            topo.soa.cap_ieq[i] = -g * *v0;
        }
        let mut il0 = vec![0.0_f64; topo.soa.ind_names.len()];
        for (i, i0) in il0.iter_mut().enumerate() {
            let ic = topo.soa.ind_ic[i];
            *i0 = if ic.is_nan() { 0.0 } else { ic };
            let r = topo.soa.ind_inductance[i] / h0;
            topo.soa.ind_req[i] = r;
            topo.soa.ind_veq[i] = -r * *i0;
        }

        let mut x = vec![0.0_f64; dim];
        for (&node, &v) in initial_conditions {
            if node > 0 && node - 1 < dim {
                x[node - 1] = v;
            }
        }
        NonlinearSolver::sync_device_voltages(topo, &x);
        linear_solver.invalidate_factorization();
        let (iters, _) = nonlinear_solver
            .solve_in_place(topo, backend, linear_solver, &mut x)
            .map_err(|e| SimError::TransientFailure {
                time: 0.0,
                reason: format!(
                    "the `uic` start point (initial conditions imposed) did not converge: {e}"
                ),
            })?;
        NonlinearSolver::ensure_finite(&x)?;

        for (i, &vc) in vc0.iter().enumerate() {
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_neg[i]);
            topo.soa.cap_v_prev[i] = vc;
            topo.soa.cap_v_prev2[i] = vc;
            topo.soa.cap_i_prev[i] = topo.soa.cap_geq[i] * (vp - vn - vc);
        }
        for (i, &il) in il0.iter().enumerate() {
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_neg[i]);
            topo.soa.ind_i_prev[i] = il;
            topo.soa.ind_i_prev2[i] = il;
            topo.soa.ind_v_prev[i] = vp - vn;
        }
        topo.soa.is_transient = false;
        Ok((x, iters))
    }

    /// Whether a source value jumps at `t` (its limits from both sides differ): rows between `t` and the next
    /// step must then not be interpolated through the pre-jump solution.
    fn sources_jump_at(soa: &DeviceSoaBuffer, t: f64, teps: f64) -> bool {
        soa.vsrc_waveforms
            .iter()
            .chain(soa.isrc_waveforms.iter())
            .any(|w| {
                let (l, r) = (w.evaluate_at(t - teps), w.evaluate_at(t + teps));
                (l - r).abs() > 1e-6 * l.abs().max(r.abs()) + 1e-15
            })
    }

    /// The earliest source breakpoint after `t` (by more than `teps`) and before `tstop`.
    fn next_source_breakpoint(soa: &DeviceSoaBuffer, t: f64, teps: f64, tstop: f64) -> Option<f64> {
        soa.vsrc_waveforms
            .iter()
            .chain(soa.isrc_waveforms.iter())
            .filter_map(|w| w.next_breakpoint(t + teps))
            .filter(|&bp| bp < tstop - teps)
            .fold(None, |acc: Option<f64>, bp| {
                Some(acc.map_or(bp, |a| a.min(bp)))
            })
    }

    /// Solves a linear circuit whose matrix (and LU factors) did not change: only the source values and the
    /// companion history currents are new.
    fn solve_linear_cached(
        &mut self,
        topo: &mut CompiledTopology,
        linear_solver: &mut AdaptiveSparseSolver,
        x: &mut [f64],
    ) -> SimResult<()> {
        let soa = &mut topo.soa;
        for (i, &val) in soa.vsrc_values.iter().enumerate() {
            soa.rhs_buf[soa.vsrc_rhs_buf_idx[i] as usize] = val;
        }
        for (i, &val) in soa.isrc_values.iter().enumerate() {
            soa.rhs_buf[soa.isrc_rhs_buf_idx[i] as usize] = val;
        }
        for i in 0..soa.cap_names.len() {
            soa.rhs_buf[soa.cap_rhs_buf_idx[i] as usize] = soa.cap_ieq[i];
        }
        for i in 0..soa.ind_names.len() {
            soa.rhs_buf[soa.ind_rhs_buf_idx[i] as usize] = soa.ind_veq[i];
        }
        // Assemble only the RHS vector (zero matrix work)
        self.rhs_scratch.fill(0.0);
        for &(row_i, start, end) in &soa.dynamic_gather_table.active_rhs {
            let mut acc = 0.0_f64;
            for term in &soa.dynamic_gather_table.rhs_terms[start as usize..end as usize] {
                acc = term
                    .sign
                    .mul_add(soa.rhs_buf[term.buffer_idx as usize], acc);
            }
            self.rhs_scratch[row_i as usize] = acc;
        }
        linear_solver.solve_cached(&self.rhs_scratch, x)
    }

    /// Largest ratio `LTE estimate / tolerance` over all capacitor voltages and inductor currents for the step
    /// to `t_new` that produced `x` (`0.0` when the history since the last restart is too short to tell).
    ///
    /// The third derivative is the third divided difference of `y_{n+1}, y_n, y_{n-1}, y_{n-2}` (`y''' ~ 6 DD3`,
    /// variable steps allowed); the LTE is `h^3 |y'''| / 12` for the trapezoid, `2 h^3 |y'''| / 9` for BDF-2
    /// and `h^2 |y''| / 2` for backward Euler (`y'' ~ 2 DD2`). The tolerance is
    /// `trtol * (reltol * max(|y_n|, |y_{n+1}|) + floor)` (see the body for the floors).
    #[allow(clippy::too_many_arguments)]
    fn lte_ratio(
        &self,
        topo: &CompiledTopology,
        x: &[f64],
        method: IntegrationMethod,
        h: f64,
        t_new: f64,
        t_hist: &[f64; 3],
        n_hist: usize,
    ) -> f64 {
        let o = &self.options;
        let soa = &topo.soa;
        let [t0, t1, t2] = *t_hist;
        let (coef, needs) = match method {
            IntegrationMethod::Trapezoidal => (0.5 * h * h * h, 3),
            IntegrationMethod::Bdf2 => (4.0 / 3.0 * h * h * h, 3),
            IntegrationMethod::Bdf1 => (h * h, 2),
        };
        if n_hist < needs {
            return 0.0;
        }
        // divided-difference weights of y_{n+1}, y_n, y_{n-1}, y_{n-2}
        let w: [f64; 4] = if needs == 3 {
            let tn = [t_new, t0, t1, t2];
            let mut w = [0.0; 4];
            for j in 0..4 {
                let mut d = 1.0;
                for k in 0..4 {
                    if k != j {
                        d *= tn[j] - tn[k];
                    }
                }
                w[j] = 1.0 / d;
            }
            w
        } else {
            let tn = [t_new, t0, t1];
            let mut w = [0.0; 4];
            for j in 0..3 {
                let mut d = 1.0;
                for k in 0..3 {
                    if k != j {
                        d *= tn[j] - tn[k];
                    }
                }
                w[j] = 1.0 / d;
            }
            w
        };
        if !w.iter().all(|v| v.is_finite()) {
            return 0.0;
        }
        // Error per step of every capacitor voltage and inductor current against `reltol * |y| + floor`. The floor
        // of a voltage is `vntol`; that of a current is `vntol` carried to the current domain by the circuit's own
        // scale (largest inductor current / largest node voltage), i.e. the current error that would move a node
        // by `vntol`, and never below `abstol`.
        let n_nodes = soa.num_nodes.min(x.len());
        let v_scale = x[..n_nodes].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let i_scale = soa
            .ind_branch_idx
            .iter()
            .fold(0.0_f64, |m, &b| m.max(x[b].abs()));
        let i_floor = if v_scale > 0.0 {
            (o.vntol * i_scale / v_scale).max(o.abstol)
        } else {
            o.abstol
        };
        let mut r = 0.0_f64;
        let mut check = |floor: f64, y: f64, y0: f64, y1: f64, y2: f64| {
            let dd = w[0] * y + w[1] * y0 + w[2] * y1 + w[3] * y2;
            let tol = o.trtol * (o.reltol * y.abs().max(y0.abs()) + floor);
            r = r.max(coef * dd.abs() / tol);
        };
        for i in 0..soa.cap_names.len() {
            let vp = DeviceSoaBuffer::node_voltage(x, soa.cap_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(x, soa.cap_n_neg[i]);
            check(
                o.vntol,
                vp - vn,
                soa.cap_v_prev[i],
                soa.cap_v_prev2[i],
                self.cap_v_prev3[i],
            );
        }
        for i in 0..soa.ind_names.len() {
            check(
                i_floor,
                x[soa.ind_branch_idx[i]],
                soa.ind_i_prev[i],
                soa.ind_i_prev2[i],
                self.ind_i_prev3[i],
            );
        }
        r
    }

    /// Stamps the companion models of `method` for a step `h` after a step `h_prev`; returns the factor of
    /// `C` (and `L`) in the companion conductance (resistance), which identifies the matrix of a linear circuit.
    ///
    /// Variable-step BDF-2 with `w = h / h_prev`:
    /// `y'_{n+1} = [(1 + 2w) / (1 + w) y_{n+1} - (1 + w) y_n + w^2 / (1 + w) y_{n-1}] / h`.
    #[inline]
    fn update_companion(
        topo: &mut CompiledTopology,
        method: IntegrationMethod,
        h: f64,
        h_prev: f64,
    ) -> f64 {
        let (a0, a1, a2, uses_rate) = match method {
            IntegrationMethod::Bdf1 => (1.0 / h, -1.0 / h, 0.0, false),
            IntegrationMethod::Trapezoidal => (2.0 / h, -2.0 / h, 0.0, true),
            IntegrationMethod::Bdf2 => {
                if h_prev > 0.0 {
                    let w = h / h_prev;
                    (
                        (1.0 + 2.0 * w) / ((1.0 + w) * h),
                        -(1.0 + w) / h,
                        w * w / ((1.0 + w) * h),
                        false,
                    )
                } else {
                    (1.0 / h, -1.0 / h, 0.0, false)
                }
            }
        };
        topo.soa.transient_g_scale = a0;
        let soa = &mut topo.soa;
        for i in 0..soa.cap_names.len() {
            let c = soa.cap_capacitance[i];
            let mut hist = c * (a1 * soa.cap_v_prev[i] + a2 * soa.cap_v_prev2[i]);
            if uses_rate {
                hist -= soa.cap_i_prev[i];
            }
            soa.cap_geq[i] = c * a0;
            soa.cap_ieq[i] = hist;
        }
        for i in 0..soa.ind_names.len() {
            let l = soa.ind_inductance[i];
            let mut hist = l * (a1 * soa.ind_i_prev[i] + a2 * soa.ind_i_prev2[i]);
            if uses_rate {
                hist -= soa.ind_v_prev[i];
            }
            soa.ind_req[i] = l * a0;
            soa.ind_veq[i] = hist;
        }
        a0
    }

    #[inline]
    fn commit_reactive_history(topo: &mut CompiledTopology, x: &[f64]) {
        for i in 0..topo.soa.cap_names.len() {
            let vp = DeviceSoaBuffer::node_voltage(x, topo.soa.cap_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(x, topo.soa.cap_n_neg[i]);
            let vc = vp - vn;
            let ic = topo.soa.cap_geq[i].mul_add(vc, topo.soa.cap_ieq[i]);
            topo.soa.cap_v_prev2[i] = topo.soa.cap_v_prev[i];
            topo.soa.cap_v_prev[i] = vc;
            topo.soa.cap_i_prev[i] = ic;
        }
        for i in 0..topo.soa.ind_names.len() {
            let br = topo.soa.ind_branch_idx[i];
            let il = x[br];
            let vp = DeviceSoaBuffer::node_voltage(x, topo.soa.ind_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(x, topo.soa.ind_n_neg[i]);
            let vl = vp - vn;
            topo.soa.ind_i_prev2[i] = topo.soa.ind_i_prev[i];
            topo.soa.ind_i_prev[i] = il;
            topo.soa.ind_v_prev[i] = vl;
        }
    }
}
