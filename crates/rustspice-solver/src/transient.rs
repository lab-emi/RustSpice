//! Time-domain Transient Integrator (`Trapezoidal`, `BDF-1`, `BDF-2`) with Constant-$h$
//! LU Factorization Caching, Secant Trajectory Prediction, and Adaptive Timestep Recovery.

use rustspice_core::{
    CompiledTopology, ComputeBackend, DeviceSoaBuffer, ResourceLimits, SimError, SimResult,
};
use serde::{Deserialize, Serialize};

use crate::linear::AdaptiveSparseSolver;
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

/// Equal Backward-Euler sub-steps used for the first output interval (see `solve_transient`).
const STARTUP_SUBSTEPS: usize = 16;

/// Configuration options for transient simulation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranOptions {
    pub method: IntegrationMethod,
    pub use_predictor: bool,
}

impl Default for TranOptions {
    fn default() -> Self {
        Self {
            method: IntegrationMethod::Trapezoidal,
            use_predictor: true,
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
}

/// High-speed Transient Integrator.
pub struct TransientSolver {
    pub options: TranOptions,
    /// Work / memory budgets enforced by `solve_transient`.
    pub limits: ResourceLimits,
    rhs_scratch: Vec<f64>,
}

impl TransientSolver {
    pub fn new(options: TranOptions) -> Self {
        Self {
            options,
            limits: ResourceLimits::default(),
            rhs_scratch: Vec::new(),
        }
    }

    /// Runs a transient analysis from `t = 0.0` to `tstop` with nominal step `tstep`.
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

        let dim = topo.soa.mna_dim;
        self.rhs_scratch.resize(dim, 0.0);

        // Budget check BEFORE any work or allocation: `tstop / tstep` can be 1e12 or 1e300.
        let est_points = (tstop / tstep).ceil() + 2.0;
        self.limits
            .check_points("transient_points", est_points, dim)?;
        let point_cap = self.limits.point_cap(dim);

        // 1. Compute initial transient operating point at t = 0.0
        topo.soa.is_transient = false;
        topo.soa.update_sources_at_time(Some(0.0));
        NonlinearSolver::check_dc_solvable_with_ic(&topo.soa, Some(initial_conditions))?;
        // Initial solution: Newton, then gmin stepping, then source stepping (as for `.op`)
        let op = nonlinear_solver.solve_op_at_current_sources(topo, backend, linear_solver)?;
        let op_iters = op.iterations;
        let mut x = op.solution;
        for (&node_id, &ic_val) in initial_conditions {
            if node_id > 0 && node_id - 1 < dim {
                x[node_id - 1] = ic_val;
            }
        }

        // Initialize capacitor voltages/currents and inductor currents/voltages at t = 0.0
        for i in 0..topo.soa.cap_names.len() {
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.cap_n_neg[i]);
            let vc0 = vp - vn;
            topo.soa.cap_v_prev[i] = vc0;
            topo.soa.cap_v_prev2[i] = vc0;
            topo.soa.cap_i_prev[i] = 0.0;
        }
        for i in 0..topo.soa.ind_names.len() {
            let br = topo.soa.ind_branch_idx[i];
            let il0 = x[br];
            let vp = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_pos[i]);
            let vn = DeviceSoaBuffer::node_voltage(&x, topo.soa.ind_n_neg[i]);
            topo.soa.ind_i_prev[i] = il0;
            topo.soa.ind_i_prev2[i] = il0;
            topo.soa.ind_v_prev[i] = vp - vn;
        }

        // `est_points <= point_cap` was verified above, so this reservation is bounded
        let est_steps = est_points as usize;
        let mut time_points = Vec::with_capacity(est_steps);
        let mut waveforms = Vec::with_capacity(est_steps);
        time_points.push(0.0);
        waveforms.push(x.clone());

        topo.soa.is_transient = true;
        linear_solver.invalidate_factorization();

        // Refresh static-pivot symbolic LU with the transient companion conductances (`2C/h`, `2L/h`)
        // stamped on the diagonal so capacitor/inductor meshes and ladders select diagonally dominant
        // pivots rather than the open-cap / short-inductor DC operating-point pivots.
        if !topo.soa.cap_names.is_empty() || !topo.soa.ind_names.is_empty() {
            Self::update_companion_full(topo, self.options.method, tstep.min(tstop), 0.0);
            backend.evaluate_devices(&mut topo.soa, &x)?;
            backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs_scratch)?;
            linear_solver.refresh_for_transient(&topo.jacobian);
            linear_solver.invalidate_factorization();
        }

        let is_linear = topo.soa.is_linear();
        let mut prev_h = 0.0_f64;
        let mut prev_method: Option<IntegrationMethod> = None;
        let mut prev_x = x.clone();
        let mut prev2_x = x.clone();
        let mut total_nr_iterations = op_iters;
        let mut lu_factorizations = 1usize;
        let mut step_index = 0usize;

        // Integer-counted output grid. Row k sits at exactly `k * tstep`; the old accumulated `t += h`
        // drifted by more than the `1e-9 * tstep` loop guard after a few thousand steps and appended a
        // 1e-20 s sliver step with garbage values on stiff meshes. `n_full` regular intervals of `tstep`
        // are followed by one genuinely shorter interval only when the tail exceeds `tol_q` steps; a
        // tail shorter than that is merged into the last regular interval, whose row is stamped `tstop`.
        let q = tstop / tstep;
        let tol_q = 1e-9 + 4.0 * f64::EPSILON * q;
        let n_full = (q + tol_q).floor() as usize;
        let tail = tstop - (n_full as f64) * tstep;
        let has_tail = tail > tol_q * tstep;
        let n_steps = n_full.saturating_add(usize::from(has_tail));

        let mut t = 0.0_f64;
        for k in 1..=n_steps {
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

            let t_start = t;
            let t_target = if k == n_steps {
                tstop
            } else {
                (k as f64) * tstep
            };
            let h_interval = if k <= n_full { tstep } else { tail };
            let startup_substeps = if self.options.method == IntegrationMethod::Bdf1 {
                1
            } else {
                STARTUP_SUBSTEPS
            };
            let startup = k == 1 && startup_substeps > 1;
            let h_nominal = if startup {
                h_interval / (startup_substeps as f64)
            } else {
                h_interval
            };
            let step_method = if startup {
                IntegrationMethod::Bdf1
            } else {
                self.options.method
            };

            // Advance from the interval start to `t_target`. Normally one step; the start-up interval uses
            // `STARTUP_SUBSTEPS` equal BDF-1 sub-steps and a Newton failure splits a step (h /= 4) and
            // re-synchronises on the grid point.
            let mut off = 0.0_f64;
            let mut h_try = h_nominal;
            loop {
                let left = h_interval - off;
                let finishing = h_try >= left * (1.0 - 1e-9);
                let mut h = if finishing { left } else { h_try };
                let t_cur = t_start + off;

                let can_reuse_linear_lu = is_linear
                    && prev_method == Some(step_method)
                    && (h - prev_h).abs() <= 1e-9 * h
                    && linear_solver.has_valid_factorization();

                let landed;
                if can_reuse_linear_lu {
                    // Ultra-fast linear constant-h path:
                    // Geq and Req are unchanged -> Jacobian matrix and LU factors are invariant!
                    // Only update companion history currents/voltages + independent sources and solve!
                    let t_next = if finishing { t_target } else { t_cur + h };
                    topo.soa.update_sources_at_time(Some(t_next));
                    Self::update_companion_rhs_only(topo, step_method, h, prev_h);

                    for (i, &val) in topo.soa.vsrc_values.iter().enumerate() {
                        topo.soa.rhs_buf[topo.soa.vsrc_rhs_buf_idx[i] as usize] = val;
                    }
                    for (i, &val) in topo.soa.isrc_values.iter().enumerate() {
                        topo.soa.rhs_buf[topo.soa.isrc_rhs_buf_idx[i] as usize] = val;
                    }
                    for i in 0..topo.soa.cap_names.len() {
                        topo.soa.rhs_buf[topo.soa.cap_rhs_buf_idx[i] as usize] =
                            topo.soa.cap_ieq[i];
                    }
                    for i in 0..topo.soa.ind_names.len() {
                        topo.soa.rhs_buf[topo.soa.ind_rhs_buf_idx[i] as usize] =
                            topo.soa.ind_veq[i];
                    }

                    // Assemble only the RHS vector (zero matrix work!)
                    self.rhs_scratch.fill(0.0);
                    for &(row_i, start, end) in &topo.soa.dynamic_gather_table.active_rhs {
                        let mut acc = 0.0_f64;
                        for term in
                            &topo.soa.dynamic_gather_table.rhs_terms[start as usize..end as usize]
                        {
                            acc = term
                                .sign
                                .mul_add(topo.soa.rhs_buf[term.buffer_idx as usize], acc);
                        }
                        self.rhs_scratch[row_i as usize] = acc;
                    }

                    linear_solver.solve_cached(&self.rhs_scratch, &mut x)?;
                    if let Err(SimError::NonFiniteSolution { unknown }) =
                        NonlinearSolver::ensure_finite(&x)
                    {
                        return Err(SimError::TransientFailure {
                            time: t_next,
                            reason: format!(
                                "non-finite value (overflow or NaN) in MNA unknown {unknown}"
                            ),
                        });
                    }
                    Self::commit_reactive_history(topo, &x);

                    prev2_x.copy_from_slice(&prev_x);
                    prev_x.copy_from_slice(&x);
                    total_nr_iterations += 1;
                    step_index += 1;
                    landed = finishing;
                } else {
                    // General nonlinear or timestep-changing path with adaptive sub-stepping retry
                    let mut sub_attempt = 0usize;
                    loop {
                        let lands_here = finishing && sub_attempt == 0;
                        let t_next = if lands_here { t_target } else { t_cur + h };
                        topo.soa.update_sources_at_time(Some(t_next));
                        Self::update_companion_full(topo, step_method, h, prev_h);

                        // Predict initial guess x for t_next
                        if self.options.use_predictor
                            && step_index >= 2
                            && sub_attempt == 0
                            && prev_h > 0.0
                        {
                            let ratio = (h / prev_h).min(1.0);
                            for i in 0..dim {
                                x[i] = prev_x[i] + ratio * (prev_x[i] - prev2_x[i]);
                            }
                            // Junction limiting (pnjlim) must be relative to the last converged step, not to
                            // the extrapolated guess: a predictor that overshoots a switching edge would
                            // otherwise start Newton unlimited, deep in the exponential.
                            NonlinearSolver::sync_device_voltages(topo, &prev_x);
                        } else {
                            x.copy_from_slice(&prev_x);
                            NonlinearSolver::sync_device_voltages(topo, &x);
                        }

                        match nonlinear_solver.solve_in_place(topo, backend, linear_solver, &mut x)
                        {
                            Ok((iters, _)) => {
                                Self::commit_reactive_history(topo, &x);
                                prev_h = h;
                                prev_method = Some(step_method);
                                prev2_x.copy_from_slice(&prev_x);
                                prev_x.copy_from_slice(&x);
                                total_nr_iterations += iters;
                                lu_factorizations += iters;
                                step_index += 1;
                                landed = lands_here;
                                break;
                            }
                            Err(e) => {
                                sub_attempt += 1;
                                if sub_attempt > 6 {
                                    return Err(SimError::TransientFailure {
                                        time: t_cur + h,
                                        reason: format!(
                                            "Step convergence failed after retries: {e}"
                                        ),
                                    });
                                }
                                h *= 0.25;
                            }
                        }
                    }
                }

                if landed {
                    t = t_target;
                    time_points.push(t);
                    waveforms.push(x.clone());
                    break;
                }
                off += h;
                h_try = h_nominal;
            }
        }

        topo.soa.is_transient = false;
        Ok(TranResult {
            time_points,
            waveforms,
            total_nr_iterations,
            lu_factorizations,
        })
    }

    #[inline]
    fn update_companion_full(
        topo: &mut CompiledTopology,
        method: IntegrationMethod,
        h: f64,
        prev_h: f64,
    ) {
        let g_scale = match method {
            IntegrationMethod::Bdf1 => 1.0 / h,
            IntegrationMethod::Trapezoidal => 2.0 / h,
            IntegrationMethod::Bdf2 => {
                if prev_h > 0.0 && (h - prev_h).abs() <= 1e-12 * h {
                    1.5 / h
                } else {
                    1.0 / h
                }
            }
        };
        topo.soa.transient_g_scale = g_scale;

        for i in 0..topo.soa.cap_names.len() {
            let c = topo.soa.cap_capacitance[i];
            let v_prev = topo.soa.cap_v_prev[i];
            let v_prev2 = topo.soa.cap_v_prev2[i];
            let i_prev = topo.soa.cap_i_prev[i];

            let (geq, ieq) = match method {
                IntegrationMethod::Bdf1 => {
                    let g = c / h;
                    (g, -g * v_prev)
                }
                IntegrationMethod::Trapezoidal => {
                    let g = 2.0 * c / h;
                    (g, -g * v_prev - i_prev)
                }
                IntegrationMethod::Bdf2 => {
                    if prev_h > 0.0 && (h - prev_h).abs() <= 1e-12 * h {
                        let g = 1.5 * c / h;
                        let hist = (c / h) * (-2.0 * v_prev + 0.5 * v_prev2);
                        (g, hist)
                    } else {
                        let g = c / h;
                        (g, -g * v_prev)
                    }
                }
            };
            topo.soa.cap_geq[i] = geq;
            topo.soa.cap_ieq[i] = ieq;
        }

        for i in 0..topo.soa.ind_names.len() {
            let l = topo.soa.ind_inductance[i];
            let i_prev = topo.soa.ind_i_prev[i];
            let i_prev2 = topo.soa.ind_i_prev2[i];
            let v_prev = topo.soa.ind_v_prev[i];

            let (req, veq) = match method {
                IntegrationMethod::Bdf1 => {
                    let r = l / h;
                    (r, -r * i_prev)
                }
                IntegrationMethod::Trapezoidal => {
                    let r = 2.0 * l / h;
                    (r, -r * i_prev - v_prev)
                }
                IntegrationMethod::Bdf2 => {
                    if prev_h > 0.0 && (h - prev_h).abs() <= 1e-12 * h {
                        let r = 1.5 * l / h;
                        let hist = (l / h) * (-2.0 * i_prev + 0.5 * i_prev2);
                        (r, hist)
                    } else {
                        let r = l / h;
                        (r, -r * i_prev)
                    }
                }
            };
            topo.soa.ind_req[i] = req;
            topo.soa.ind_veq[i] = veq;
        }
    }

    #[inline]
    fn update_companion_rhs_only(
        topo: &mut CompiledTopology,
        method: IntegrationMethod,
        h: f64,
        prev_h: f64,
    ) {
        for i in 0..topo.soa.cap_names.len() {
            let geq = topo.soa.cap_geq[i];
            let v_prev = topo.soa.cap_v_prev[i];
            let i_prev = topo.soa.cap_i_prev[i];
            topo.soa.cap_ieq[i] = match method {
                IntegrationMethod::Bdf1 => -geq * v_prev,
                IntegrationMethod::Trapezoidal => -geq * v_prev - i_prev,
                IntegrationMethod::Bdf2 => {
                    let c = topo.soa.cap_capacitance[i];
                    let v_prev2 = topo.soa.cap_v_prev2[i];
                    if prev_h > 0.0 {
                        (c / h) * (-2.0 * v_prev + 0.5 * v_prev2)
                    } else {
                        -geq * v_prev
                    }
                }
            };
        }

        for i in 0..topo.soa.ind_names.len() {
            let req = topo.soa.ind_req[i];
            let i_prev = topo.soa.ind_i_prev[i];
            let v_prev = topo.soa.ind_v_prev[i];
            topo.soa.ind_veq[i] = match method {
                IntegrationMethod::Bdf1 => -req * i_prev,
                IntegrationMethod::Trapezoidal => -req * i_prev - v_prev,
                IntegrationMethod::Bdf2 => {
                    let l = topo.soa.ind_inductance[i];
                    let i_prev2 = topo.soa.ind_i_prev2[i];
                    if prev_h > 0.0 {
                        (l / h) * (-2.0 * i_prev + 0.5 * i_prev2)
                    } else {
                        -req * i_prev
                    }
                }
            };
        }
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
