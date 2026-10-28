//! Accelerated Nonlinear Solver:
//! - Damped Newton-Raphson with SPICE3f5 `pnjlim` / `fetlim`
//! - Linear Circuit 1-Step Fast-Path
//! - Early Residual Convergence Termination, Contraction-Rate Acceptance (transient) & Shamanskii Jacobian LU Reuse
//! - Secant Trajectory Predictor for DC Sweeps and Transient Time-Stepping
//! - Homotopy Continuation (`Gmin Stepping` & `Source Stepping`)

use rustspice_backend_cpu::DIODE_EXP_ARG_MAX;
use rustspice_core::{
    CompiledTopology, ComputeBackend, DeviceSoaBuffer, ResourceLimits, SimError, SimResult,
};
use serde::{Deserialize, Serialize};

use crate::linear::AdaptiveSparseSolver;

/// Default number of consecutive binding iterations after which the voltage box is released.
pub const BOX_RELEASE_ITERS: usize = 16;

/// Configuration options for the Damped Newton-Raphson solver.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NrOptions {
    pub max_iters: usize,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    /// Relative part of the KCL residual criterion: row `i` is converged when
    /// `|r_i| <= abstol + res_reltol * s_i` with `s_i = |rhs_i| + sum_j |J_ij x_j|` the magnitude of the
    /// terms that were summed to form `r_i`. A purely absolute `abstol` is below the f64 rounding
    /// noise (`~1e-16 * s_i`) of any row that carries more than ~1e3 A, so it can never be met there.
    pub res_reltol: f64,
    /// The source-derived voltage box that stabilizes the first Newton iterations of circuits without
    /// MOSFET step limiting is released for the rest of the solve once it has been *binding* (moved a
    /// node voltage by more than the step tolerance) on this many consecutive iterations: at that point
    /// the box, not the circuit, is what keeps the residual from vanishing (a rail, rectifier or
    /// current-driven node outside the source range).
    pub box_release_iters: usize,
    pub enable_shamanskii: bool,
    pub enable_homotopy: bool,
}

impl Default for NrOptions {
    fn default() -> Self {
        Self {
            max_iters: 150,
            reltol: 1e-6,
            vntol: 1e-10,
            abstol: 1e-12,
            res_reltol: 1e-12,
            box_release_iters: BOX_RELEASE_ITERS,
            enable_shamanskii: true,
            enable_homotopy: true,
        }
    }
}

/// Converged DC Operating Point result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DcOpResult {
    pub solution: Vec<f64>,
    pub iterations: usize,
    pub max_residual: f64,
}

/// Converged DC Sweep result across a range of source values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DcSweepResult {
    pub sweep_values: Vec<f64>,
    pub solutions: Vec<Vec<f64>>,
    pub total_iterations: usize,
}

/// Pre-allocated Newton-Raphson solver workspace guaranteeing zero heap allocations in the hot loop.
pub struct NonlinearSolver {
    pub options: NrOptions,
    /// Work / memory budgets enforced by `solve_dc_sweep`.
    pub limits: ResourceLimits,
    rhs: Vec<f64>,
    next_x: Vec<f64>,
    residual_vec: Vec<f64>,
    residual_scale: Vec<f64>,
    delta_x: Vec<f64>,
}

impl NonlinearSolver {
    /// Creates a new [`NonlinearSolver`] pre-allocated for MNA dimension `dim`.
    pub fn new(dim: usize, options: NrOptions) -> Self {
        Self {
            options,
            limits: ResourceLimits::default(),
            rhs: vec![0.0; dim],
            next_x: vec![0.0; dim],
            residual_vec: vec![0.0; dim],
            residual_scale: vec![0.0; dim],
            delta_x: vec![0.0; dim],
        }
    }

    /// Ensures internal scratch buffers match `dim`.
    #[inline]
    pub fn ensure_dim(&mut self, dim: usize) {
        if self.rhs.len() != dim {
            self.rhs.resize(dim, 0.0);
            self.next_x.resize(dim, 0.0);
            self.residual_vec.resize(dim, 0.0);
            self.residual_scale.resize(dim, 0.0);
            self.delta_x.resize(dim, 0.0);
        }
    }

    /// Fails with [`SimError::NonFiniteSolution`] if any unknown is infinite or NaN (overflow in the linear
    /// solve); the nonlinear path already rejects non-finite Newton steps.
    #[inline]
    pub fn ensure_finite(x: &[f64]) -> SimResult<()> {
        // Branch-free scan (all-ones exponent <=> inf or NaN) so the per-step cost on the linear
        // fast paths stays at a few vector ops; the index is only searched on the failure path.
        let mut bad = false;
        for v in x {
            bad |= (v.to_bits() & 0x7ff0_0000_0000_0000) == 0x7ff0_0000_0000_0000;
        }
        if !bad {
            return Ok(());
        }
        let unknown = x.iter().position(|v| !v.is_finite()).unwrap_or(0);
        Err(SimError::NonFiniteSolution { unknown })
    }

    /// Fused `y = J x` and `s_i = sum_j |J_ij x_j|` in one pass over the CSC matrix.
    #[inline]
    fn spmv_with_magnitude(
        jac: &rustspice_core::CscMatrix<f64>,
        x: &[f64],
        y: &mut [f64],
        mag: &mut [f64],
    ) {
        y.fill(0.0);
        mag.fill(0.0);
        for (col, &xj) in x.iter().enumerate().take(jac.ncols) {
            if xj == 0.0 {
                continue;
            }
            for idx in jac.col_ptr[col]..jac.col_ptr[col + 1] {
                let term = jac.values[idx] * xj;
                let row = jac.row_idx[idx];
                y[row] += term;
                mag[row] += term.abs();
            }
        }
    }

    /// Rejects an accepted iterate `x` whose diode junction needs the linear continuation of the
    /// Shockley exponential (`v_d / (n V_t) > DIODE_EXP_ARG_MAX`): the numbers there are not the
    /// model's, so the operating point is reported as an error instead of being returned silently.
    /// The junction voltage is read from `x` itself, not from the limited evaluation point, so a
    /// limiter that is still climbing toward an out-of-range solution cannot hide it.
    fn check_diode_exponent_range(soa: &DeviceSoaBuffer, x: &[f64]) -> SimResult<()> {
        for i in 0..soa.diode_names.len() {
            let va = DeviceSoaBuffer::node_voltage(x, soa.diode_anode[i]);
            let vc = DeviceSoaBuffer::node_voltage(x, soa.diode_cathode[i]);
            let vd = va - vc;
            let arg = vd / (soa.diode_n[i] * soa.temp_vt);
            if arg > DIODE_EXP_ARG_MAX {
                return Err(SimError::DeviceRangeExceeded(format!(
                    "diode `{}` junction voltage {:.6e} V is {:.1} thermal-voltage units (exponent argument limit {})",
                    soa.diode_names[i], vd, arg, DIODE_EXP_ARG_MAX
                )));
            }
        }
        Ok(())
    }

    /// Synchronizes internal diode junction voltages `diode_vd` with a node voltage vector `x`
    /// (the reference point of the next junction-limiting step).
    #[inline]
    pub fn sync_device_voltages(topo: &mut CompiledTopology, x: &[f64]) {
        for i in 0..topo.soa.diode_names.len() {
            let va = rustspice_core::DeviceSoaBuffer::node_voltage(x, topo.soa.diode_anode[i]);
            let vc = rustspice_core::DeviceSoaBuffer::node_voltage(x, topo.soa.diode_cathode[i]);
            topo.soa.diode_vd[i] = va - vc;
        }
    }

    /// Largest contraction `|dx_k| / |dx_(k-1)|` (in units of the step tolerance) for which a transient Newton
    /// iterate is accepted from the rate of convergence alone.
    const RATE_MAX_CONTRACTION: f64 = 0.1;
    /// Error bound, as a fraction of the step tolerance, under which that iterate is accepted. At 1.0 the drift of
    /// slowly converging diode circuits (boost converters) reached 6x the error of the residual criterion.
    const RATE_ACCEPT_FRACTION: f64 = 0.1;

    /// Solves the nonlinear (or linear) MNA system starting from `x` in place.
    pub fn solve_in_place(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        x: &mut [f64],
    ) -> SimResult<(usize, f64)> {
        let dim = topo.soa.mna_dim;
        self.ensure_dim(dim);

        // Fast Path: Purely linear circuit (R, G, C, L, V, I only) converges in 1 linear solve!
        if topo.soa.is_linear() {
            backend.evaluate_devices(&mut topo.soa, x)?;
            backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs)?;
            linear_solver.factorize_and_solve(&topo.jacobian, &self.rhs, x)?;
            Self::ensure_finite(x)?;
            return Ok((1, 0.0));
        }

        let num_nodes = topo.soa.num_nodes;
        let v_min = topo
            .soa
            .vsrc_values
            .iter()
            .fold(0.0_f64, |acc, &v| acc.min(v));
        let v_max = topo
            .soa
            .vsrc_values
            .iter()
            .fold(0.0_f64, |acc, &v| acc.max(v));
        let mut v_box = Some(
            if !topo.soa.is_transient
                && topo.soa.isrc_values.is_empty()
                && topo.soa.vccs_gm.is_empty()
                && (v_max - v_min) > 0.0
            {
                (v_min, v_max)
            } else {
                let span = (v_max - v_min).max(5.0);
                let (mut lo, mut hi) = (v_min - span, v_max + span);
                if topo.soa.is_transient {
                    // A transient step starts from (an extrapolation of) the last accepted solution, which may lie
                    // outside the source range (charge pumps, boost converters): keep the box around it, or every
                    // such step spends `box_release_iters` iterations pinned to the box.
                    for &v in &x[..num_nodes.min(x.len())] {
                        lo = lo.min(v - span);
                        hi = hi.max(v + span);
                    }
                }
                (lo, hi)
            },
        );
        let mut box_binding_streak = 0usize;

        let mut prev_res_norm = f64::INFINITY;
        let mut prev_step_converged = false;
        let mut last_step_max = f64::INFINITY;
        // size of the previous Newton update in units of the step tolerance
        let mut prev_step_norm = f64::INFINITY;

        for iter in 1..=self.options.max_iters {
            backend.evaluate_devices(&mut topo.soa, x)?;
            backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs)?;

            // Compute exact KCL residual vector r = rhs - J * x together with the magnitude
            // s_i = |rhs_i| + sum_j |J_ij x_j| of the terms summed into each row.
            Self::spmv_with_magnitude(
                &topo.jacobian,
                x,
                &mut self.residual_vec,
                &mut self.residual_scale,
            );
            let mut max_res = 0.0_f64;
            let mut res_ok = true;
            for i in 0..dim {
                let r = self.rhs[i] - self.residual_vec[i];
                self.residual_vec[i] = r;
                let ar = r.abs();
                max_res = max_res.max(ar);
                let scale = self.residual_scale[i] + self.rhs[i].abs();
                let tol = self.options.abstol + self.options.res_reltol * scale;
                if ar > tol || ar.is_nan() {
                    res_ok = false;
                }
            }

            // Early convergence check: if previous step Δx was within voltage tolerance
            // AND current exact KCL residual is within tolerance, x is already converged!
            // A step limiter (`pnjlim`) or a model clamp (diode exponent saturation) that changed the
            // linearisation point makes `max_res` the residual of the ALTERED companion model, not of
            // the circuit: never accept such an iterate as converged.
            let limited = topo.soa.limiter_active;
            if iter > 1 && prev_step_converged && res_ok && !limited {
                Self::check_diode_exponent_range(&topo.soa, x)?;
                return Ok((iter - 1, max_res));
            }

            // Shamanskii / Modified Newton step: reuse existing LU factors when residual is shrinking fast
            let used_shamanskii = if self.options.enable_shamanskii
                && iter >= 2
                && linear_solver.has_valid_factorization()
                && max_res < 0.05 * prev_res_norm
                && last_step_max < 1e-3
            {
                if linear_solver
                    .solve_cached(&self.residual_vec, &mut self.delta_x)
                    .is_ok()
                {
                    for (i, &xi) in x.iter().enumerate().take(dim) {
                        self.next_x[i] = xi + self.delta_x[i];
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if !used_shamanskii {
                // Residual form: x+ = x + J^-1 (b - J x). Identical to solving J x+ = b in exact
                // arithmetic, but LU round-off then perturbs only the (small) correction instead of the
                // iterate itself, so Newton can reach the residual floor instead of cycling at the
                // factorisation's noise level.
                linear_solver.factorize_and_solve(
                    &topo.jacobian,
                    &self.residual_vec,
                    &mut self.delta_x,
                )?;
                for (i, &xi) in x.iter().enumerate().take(dim) {
                    self.next_x[i] = xi + self.delta_x[i];
                }
            }

            // Check voltage/branch step convergence
            let mut step_ok = true;
            let mut box_binding = false;
            let mut max_step = 0.0_f64;
            let mut step_norm = 0.0_f64;
            for (i, &xi) in x.iter().enumerate().take(dim) {
                let raw_dx = self.next_x[i] - xi;
                if !raw_dx.is_finite() {
                    return Err(SimError::ConvergenceFailure {
                        iterations: iter,
                        residual: f64::NAN,
                    });
                }
                let dx_abs = raw_dx.abs();
                max_step = max_step.max(dx_abs);
                let ref_val = self.next_x[i].abs().max(xi.abs());
                let tol = self.options.reltol * ref_val + self.options.vntol;
                if dx_abs > tol {
                    step_ok = false;
                }
                step_norm = step_norm.max(dx_abs / tol);
                if i < num_nodes {
                    if let Some((lo, hi)) = v_box {
                        let raw = self.next_x[i];
                        let projected = raw.clamp(lo, hi);
                        if (raw - projected).abs() > tol {
                            box_binding = true;
                        }
                        self.next_x[i] = projected;
                    }
                }
            }
            if box_binding {
                box_binding_streak += 1;
                if box_binding_streak >= self.options.box_release_iters {
                    v_box = None;
                }
            } else {
                box_binding_streak = 0;
            }

            x.copy_from_slice(&self.next_x);
            prev_res_norm = max_res;
            // a projection onto the box moved the iterate away from the update the step test judged
            prev_step_converged = step_ok && !box_binding;
            last_step_max = max_step;

            if step_ok && res_ok && max_step <= self.options.vntol && !limited && !box_binding {
                Self::check_diode_exponent_range(&topo.soa, x)?;
                return Ok((iter, max_res));
            }
            // Transient: once the updates contract by `theta` per iteration, the error left in the new iterate is
            // at most `theta / (1 - theta)` times the last update. When that bound is a small fraction of the step
            // tolerance the iterate is accepted without the evaluation that would confirm its residual.
            if topo.soa.is_transient && iter >= 2 && !limited && !box_binding {
                let theta = step_norm / prev_step_norm;
                if theta < Self::RATE_MAX_CONTRACTION
                    && theta / (1.0 - theta) * step_norm <= Self::RATE_ACCEPT_FRACTION
                {
                    Self::check_diode_exponent_range(&topo.soa, x)?;
                    return Ok((iter, max_res));
                }
            }
            prev_step_norm = step_norm;
        }

        Err(SimError::ConvergenceFailure {
            iterations: self.options.max_iters,
            residual: prev_res_norm,
        })
    }

    /// Computes the DC Operating Point (`.op`), falling back to `Gmin Stepping` and
    /// `Source Stepping` homotopy continuation if direct Newton-Raphson fails.
    pub fn solve_dc_op(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        topo.soa.is_transient = false;
        topo.soa.update_sources_at_time(None);
        Self::check_dc_solvable(&topo.soa)?;
        self.solve_op_at_current_sources(topo, backend, linear_solver)
    }

    /// DC operating point of the circuit with its independent sources at their CURRENT values
    /// (`vsrc_values` / `isrc_values`) and capacitors open: Newton from zero, then `Gmin Stepping`,
    /// then `Source Stepping`. Used by `.op`, by the transient initial solution (sources at
    /// `t = 0`) and as the per-point recovery of `.dc` sweeps.
    pub fn solve_op_at_current_sources(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        if !topo.soa.nodeset_nodes.is_empty() {
            // `.nodeset`: solve with the nodes held at their guesses, then release them and continue from there
            topo.soa.set_nodeset_clamps(true);
            let guided = self.solve_op_unguided(topo, backend, linear_solver);
            topo.soa.set_nodeset_clamps(false);
            if let Ok(g) = guided {
                let mut x = g.solution;
                Self::sync_device_voltages(topo, &x);
                if let Ok((iters, res)) = self.solve_in_place(topo, backend, linear_solver, &mut x)
                {
                    return Ok(DcOpResult {
                        solution: x,
                        iterations: g.iterations + iters,
                        max_residual: res,
                    });
                }
            }
        }
        self.solve_op_unguided(topo, backend, linear_solver)
    }

    /// [`Self::solve_op_at_current_sources`] without `.nodeset` guidance.
    fn solve_op_unguided(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        let dim = topo.soa.mna_dim;
        let mut x = vec![0.0_f64; dim];
        Self::sync_device_voltages(topo, &x);

        if let Ok((iters, res)) = self.solve_in_place(topo, backend, linear_solver, &mut x) {
            return Ok(DcOpResult {
                solution: x,
                iterations: iters,
                max_residual: res,
            });
        }

        if !self.options.enable_homotopy {
            return Err(SimError::ConvergenceFailure {
                iterations: self.options.max_iters,
                residual: f64::NAN,
            });
        }

        // Homotopy Fallback 1: Gmin Stepping (1e-2 S -> 1e-12 S)
        if let Ok(res) = self.solve_with_gmin_stepping(topo, backend, linear_solver) {
            return Ok(res);
        }

        // Homotopy Fallback 2: Source Stepping (0.1 -> 1.0)
        self.solve_with_source_stepping(topo, backend, linear_solver)
    }

    /// Detects a DC problem that has NO solution because of KCL: a connected island of elements that
    /// carry DC current (R, L, V, diode, MOS channel) and does not contain ground, yet receives a net
    /// non-zero current from independent current sources (e.g. a node touched only by a current
    /// source and capacitors). `gmin` would regularise such a circuit into `I / gmin` volts.
    /// Returns `(island nodes, net injected current)`; islands touched by a VCCS output are skipped.
    pub fn find_dc_current_cutset(
        soa: &rustspice_core::DeviceSoaBuffer,
    ) -> Option<(Vec<usize>, f64)> {
        rustspice_core::soa::find_dc_current_cutset(soa)
    }

    /// Refuses a DC problem that has no solution (see [`Self::find_dc_current_cutset`]).
    pub fn check_dc_solvable(soa: &rustspice_core::DeviceSoaBuffer) -> SimResult<()> {
        Self::check_dc_solvable_with_ic(soa, None)
    }

    /// As [`Self::check_dc_solvable`], but an island whose every node has an `.ic` value is accepted
    /// (the user supplied its start state, so no DC solution is needed there).
    pub fn check_dc_solvable_with_ic(
        soa: &rustspice_core::DeviceSoaBuffer,
        ic: Option<&rustc_hash::FxHashMap<usize, f64>>,
    ) -> SimResult<()> {
        if let Some((nodes, inj)) = Self::find_dc_current_cutset(soa) {
            if let Some(ic) = ic {
                if nodes.iter().all(|k| ic.contains_key(k)) {
                    return Ok(());
                }
            }
            let names: Vec<&str> = nodes
                .iter()
                .take(4)
                .map(|&k| soa.node_names.get(k).map(String::as_str).unwrap_or("?"))
                .collect();
            return Err(SimError::TopologyError(format!(
                "no DC solution: node(s) {}{} have no DC path to ground and receive a net {:.6e} A from independent current sources (KCL cannot be satisfied)",
                names.join(", "),
                if nodes.len() > 4 { ", ..." } else { "" },
                inj
            )));
        }
        Ok(())
    }

    /// Dynamic `gmin` stepping: solve with a large shunt conductance on every node (the circuit is then
    /// almost linear), and relax it towards the nominal `gmin`, each stage starting from the previous
    /// solution. A stage that fails to converge is retried with a smaller reduction of `gmin`.
    fn solve_with_gmin_stepping(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        let orig_gmin = topo.soa.gmin;

        let dim = topo.soa.mna_dim;
        let mut x = vec![0.0_f64; dim];
        Self::sync_device_voltages(topo, &x);

        // first stage: gmin = 1e-2 S must converge on its own
        let mut g = 1e-2_f64;
        topo.soa.gmin = g;
        let (mut total_iters, mut last_res) =
            match self.solve_in_place(topo, backend, linear_solver, &mut x) {
                Ok(v) => v,
                Err(e) => {
                    topo.soa.gmin = orig_gmin;
                    return Err(e);
                }
            };
        // geometric reduction per stage (1e-2 = the classic decade-pair schedule); shrunk on failure
        let mut ratio = 1e-2_f64;
        let mut x_good = x.clone();
        let mut stages = 0usize;
        while g > orig_gmin {
            stages += 1;
            if stages > Self::MAX_HOMOTOPY_STAGES {
                topo.soa.gmin = orig_gmin;
                return Err(SimError::ConvergenceFailure {
                    iterations: total_iters,
                    residual: f64::NAN,
                });
            }
            let next = (g * ratio).max(orig_gmin);
            topo.soa.gmin = next;
            x.copy_from_slice(&x_good);
            Self::sync_device_voltages(topo, &x);
            match self.solve_in_place(topo, backend, linear_solver, &mut x) {
                Ok((it, r)) => {
                    total_iters += it;
                    last_res = r;
                    g = next;
                    x_good.copy_from_slice(&x);
                }
                Err(e) => {
                    ratio = ratio.sqrt();
                    if ratio > 0.9 {
                        topo.soa.gmin = orig_gmin;
                        return Err(e);
                    }
                }
            }
        }

        topo.soa.gmin = orig_gmin;
        Ok(DcOpResult {
            solution: x,
            iterations: total_iters,
            max_residual: last_res,
        })
    }

    /// Hard cap on homotopy stages (each stage is one bounded Newton solve).
    const MAX_HOMOTOPY_STAGES: usize = 200;

    /// Source stepping: ramp every independent source from 0 to its value; a failing step is retried
    /// with half the increment.
    fn solve_with_source_stepping(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        // Scale the source values the caller set up (DC values for `.op`, `t = 0` values for the
        // transient start, the current sweep value for `.dc` recovery).
        let orig_v: Vec<f64> = topo.soa.vsrc_values.iter().copied().collect();
        let orig_i: Vec<f64> = topo.soa.isrc_values.iter().copied().collect();

        let dim = topo.soa.mna_dim;
        let mut x = vec![0.0_f64; dim];
        Self::sync_device_voltages(topo, &x);
        let mut x_good = x.clone();

        let mut total_iters = 0usize;
        let mut last_res = 0.0;
        let mut s_good = 0.0_f64;
        let mut ds = 0.1_f64;
        let mut stages = 0usize;
        let mut failure: Option<SimError> = None;

        while s_good < 1.0 - 1e-12 {
            stages += 1;
            if stages > Self::MAX_HOMOTOPY_STAGES {
                failure = Some(SimError::ConvergenceFailure {
                    iterations: total_iters,
                    residual: f64::NAN,
                });
                break;
            }
            let s = if s_good + ds > 1.0 - 1e-9 {
                1.0
            } else {
                s_good + ds
            };
            for (idx, &v) in orig_v.iter().enumerate() {
                let vs = v * s;
                topo.soa.vsrc_values[idx] = vs;
                topo.soa.rhs_buf[topo.soa.vsrc_rhs_buf_idx[idx] as usize] = vs;
            }
            for (idx, &c) in orig_i.iter().enumerate() {
                let cs = c * s;
                topo.soa.isrc_values[idx] = cs;
                topo.soa.rhs_buf[topo.soa.isrc_rhs_buf_idx[idx] as usize] = cs;
            }
            x.copy_from_slice(&x_good);
            Self::sync_device_voltages(topo, &x);
            match self.solve_in_place(topo, backend, linear_solver, &mut x) {
                Ok((it, r)) => {
                    total_iters += it;
                    last_res = r;
                    s_good = s;
                    x_good.copy_from_slice(&x);
                }
                Err(e) => {
                    ds *= 0.5;
                    if ds < 1e-4 {
                        failure = Some(e);
                        break;
                    }
                }
            }
        }

        // restore the caller's source values (also on failure)
        for (idx, &v) in orig_v.iter().enumerate() {
            topo.soa.vsrc_values[idx] = v;
            topo.soa.rhs_buf[topo.soa.vsrc_rhs_buf_idx[idx] as usize] = v;
        }
        for (idx, &c) in orig_i.iter().enumerate() {
            topo.soa.isrc_values[idx] = c;
            topo.soa.rhs_buf[topo.soa.isrc_rhs_buf_idx[idx] as usize] = c;
        }
        if let Some(e) = failure {
            return Err(e);
        }
        Ok(DcOpResult {
            solution: x,
            iterations: total_iters,
            max_residual: last_res,
        })
    }

    /// Values of a `.dc` sweep `start, start + step, ...` up to `stop`, budget-checked before allocation.
    pub fn sweep_grid(&self, start: f64, stop: f64, step: f64, dim: usize) -> SimResult<Vec<f64>> {
        if step == 0.0 {
            return Err(SimError::TopologyError(
                "DC sweep step cannot be zero".to_string(),
            ));
        }
        if !(start.is_finite() && stop.is_finite() && step.is_finite()) {
            return Err(SimError::TopologyError(format!(
                "DC sweep bounds must be finite (start = {start}, stop = {stop}, step = {step})"
            )));
        }
        // Budget check BEFORE any allocation: the point count is known in closed form.
        self.limits.check_points(
            "dc_sweep_points",
            (stop - start).abs() / step.abs() + 2.0,
            dim,
        )?;
        let point_cap = self.limits.point_cap(dim);
        // Integer-counted sweep grid: the point count is derived once from |stop - start| / |step| instead
        // of from the accumulated value, whose rounding drift exceeded the old `stop + 1e-12 * step` guard
        // after ~100 additions and silently dropped the last point (`.dc V1 0 5 1m` wrote 5000 of 5001
        // points). A last point within `tol_q` steps of `stop` is kept because decimal steps such as 0.1 do
        // not divide the span exactly in binary. The abscissae keep the historical `cur += step` values
        // (bit-identical to the previous output and to ngspice's own sweep values).
        let dir = if stop >= start { 1.0 } else { -1.0 };
        let step_signed = step.abs() * dir;
        let q = (stop - start).abs() / step.abs();
        let tol_q = 1e-9 + 4.0 * f64::EPSILON * q;
        let mut sweep_values = Vec::new();
        if !q.is_nan() {
            let n_last = (q + tol_q).floor() as usize;
            let mut cur = start;
            for k in 0..=n_last {
                if sweep_values.len() as u64 >= point_cap {
                    return Err(SimError::ResourceLimit {
                        resource: "dc_sweep_points",
                        requested: format!("more than {point_cap} sweep points"),
                        limit: format!("{point_cap} points (max_points / max_result_bytes)"),
                        hint: "enlarge the step or raise --max-points / --max-result-mib"
                            .to_string(),
                    });
                }
                sweep_values.push(cur);
                let next = cur + step_signed;
                if k < n_last && next == cur {
                    return Err(SimError::TopologyError(format!(
                        "DC sweep step {step:e} is below the floating-point resolution at {cur:e}: the sweep cannot advance"
                    )));
                }
                cur = next;
            }
        }
        Ok(sweep_values)
    }

    /// Executes a `.dc` parameter/source sweep using a **Secant Trajectory Predictor**
    /// and constant-Jacobian LU reuse for linear circuits.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_dc_sweep(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        source_name: &str,
        start: f64,
        stop: f64,
        step: f64,
    ) -> SimResult<DcSweepResult> {
        topo.soa.is_transient = false;
        topo.soa.update_sources_at_time(None);

        let dim = topo.soa.mna_dim;
        self.ensure_dim(dim);
        let sweep_values = self.sweep_grid(start, stop, step, dim)?;

        let orig_vsrc_waveforms = topo.soa.vsrc_waveforms.clone();
        let orig_isrc_waveforms = topo.soa.isrc_waveforms.clone();

        let res = (|| -> SimResult<DcSweepResult> {
            let mut solutions = Vec::with_capacity(sweep_values.len());
            let mut x = vec![0.0_f64; dim];
            let mut prev_x = vec![0.0_f64; dim];
            let mut prev2_x = vec![0.0_f64; dim];
            let mut total_iterations = 0usize;

            let is_linear = topo.soa.is_linear();
            let sweeps_current_source = topo
                .soa
                .isrc_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(source_name));
            if !sweeps_current_source {
                Self::check_dc_solvable(&topo.soa)?;
            }

            for (step_idx, &val) in sweep_values.iter().enumerate() {
                topo.soa.set_source_dc_value(source_name, val)?;
                if sweeps_current_source {
                    Self::check_dc_solvable(&topo.soa)?;
                }

                if is_linear && step_idx > 0 && linear_solver.has_valid_factorization() {
                    // Linear DC Sweep Fast-Path: Jacobian is invariant, only RHS source changed!
                    backend.evaluate_devices(&mut topo.soa, &x)?;
                    backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs)?;
                    linear_solver.solve_cached(&self.rhs, &mut x)?;
                    Self::ensure_finite(&x)?;
                    total_iterations += 1;
                    solutions.push(x.clone());
                    continue;
                }

                // Secant Trajectory Predictor for nonlinear sweeps
                if step_idx >= 2 {
                    for i in 0..dim {
                        x[i] = 2.0_f64.mul_add(prev_x[i], -prev2_x[i]);
                    }
                    // Junction limiting is relative to the last converged point, not the extrapolation
                    Self::sync_device_voltages(topo, &prev_x);
                }

                let iters = match self.solve_in_place(topo, backend, linear_solver, &mut x) {
                    Ok((iters, _)) => iters,
                    Err(e) => {
                        // Per-point recovery (as for `.op`): restart from zero with gmin / source stepping.
                        if !self.options.enable_homotopy {
                            return Err(e);
                        }
                        match self.solve_op_at_current_sources(topo, backend, linear_solver) {
                            Ok(r) => {
                                x.copy_from_slice(&r.solution);
                                r.iterations
                            }
                            Err(_) => return Err(e),
                        }
                    }
                };
                total_iterations += iters;

                prev2_x.copy_from_slice(&prev_x);
                prev_x.copy_from_slice(&x);
                solutions.push(x.clone());
            }

            Ok(DcSweepResult {
                sweep_values,
                solutions,
                total_iterations,
            })
        })();

        // Restore the original source waveforms and DC values so subsequent `.op` / `.tran` analyses
        // run with the deck's original source specification rather than the last sweep value.
        topo.soa.vsrc_waveforms = orig_vsrc_waveforms;
        topo.soa.isrc_waveforms = orig_isrc_waveforms;
        topo.soa.update_sources_at_time(None);

        res
    }
}
