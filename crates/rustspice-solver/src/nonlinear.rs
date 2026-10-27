//! Accelerated Nonlinear Solver:
//! - Damped Newton-Raphson with SPICE3f5 `pnjlim` / `fetlim`
//! - Linear Circuit 1-Step Fast-Path
//! - Early Residual Convergence Termination & Shamanskii Jacobian LU Reuse
//! - Secant Trajectory Predictor for DC Sweeps and Transient Time-Stepping
//! - Homotopy Continuation (`Gmin Stepping` & `Source Stepping`)

use rustspice_core::{CompiledTopology, ComputeBackend, SimError, SimResult};
use serde::{Deserialize, Serialize};

use crate::linear::AdaptiveSparseSolver;

/// Configuration options for the Damped Newton-Raphson solver.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NrOptions {
    pub max_iters: usize,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
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
    rhs: Vec<f64>,
    next_x: Vec<f64>,
    residual_vec: Vec<f64>,
    delta_x: Vec<f64>,
}

impl NonlinearSolver {
    /// Creates a new [`NonlinearSolver`] pre-allocated for MNA dimension `dim`.
    pub fn new(dim: usize, options: NrOptions) -> Self {
        Self {
            options,
            rhs: vec![0.0; dim],
            next_x: vec![0.0; dim],
            residual_vec: vec![0.0; dim],
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
            self.delta_x.resize(dim, 0.0);
        }
    }

    /// Synchronizes internal diode junction voltages `diode_vd` with a predicted node voltage vector `x`.
    #[inline]
    pub fn sync_device_voltages(topo: &mut CompiledTopology, x: &[f64]) {
        for i in 0..topo.soa.diode_names.len() {
            let va = rustspice_core::DeviceSoaBuffer::node_voltage(x, topo.soa.diode_anode[i]);
            let vc = rustspice_core::DeviceSoaBuffer::node_voltage(x, topo.soa.diode_cathode[i]);
            topo.soa.diode_vd[i] = va - vc;
        }
    }

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
        let (v_clamp_min, v_clamp_max) = if !topo.soa.is_transient
            && topo.soa.isrc_values.is_empty()
            && topo.soa.vccs_gm.is_empty()
            && (v_max - v_min) > 0.0
        {
            (v_min, v_max)
        } else {
            let span = (v_max - v_min).max(5.0);
            (v_min - span, v_max + span)
        };

        let mut prev_res_norm = f64::INFINITY;
        let mut prev_step_converged = false;
        let mut last_step_max = f64::INFINITY;

        for iter in 1..=self.options.max_iters {
            backend.evaluate_devices(&mut topo.soa, x)?;
            backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs)?;

            // Compute exact KCL residual vector: r = rhs - J * x
            topo.jacobian.spmv(x, &mut self.residual_vec);
            let mut max_res = 0.0_f64;
            for i in 0..dim {
                let r = self.rhs[i] - self.residual_vec[i];
                self.residual_vec[i] = r;
                max_res = max_res.max(r.abs());
            }

            // Early convergence check: if previous step Δx was within voltage tolerance
            // AND current exact KCL residual is within abstol, x is already converged!
            if iter > 1 && prev_step_converged && max_res <= self.options.abstol {
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
                linear_solver.factorize_and_solve(&topo.jacobian, &self.rhs, &mut self.next_x)?;
            }

            // Check voltage/branch step convergence & project unbounded saturation tangent jumps onto rails
            let mut step_ok = true;
            let mut max_step = 0.0_f64;
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
                if i < num_nodes {
                    self.next_x[i] = self.next_x[i].clamp(v_clamp_min, v_clamp_max);
                }
            }

            x.copy_from_slice(&self.next_x);
            prev_res_norm = max_res;
            prev_step_converged = step_ok;
            last_step_max = max_step;

            if step_ok && max_res <= self.options.abstol && max_step <= self.options.vntol {
                return Ok((iter, max_res));
            }
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

        let gmin_schedule = [1e-2, 1e-4, 1e-6, 1e-8, 1e-10, orig_gmin];
        let mut total_iters = 0usize;
        let mut last_res = 0.0;

        for &gmin_val in &gmin_schedule {
            topo.soa.gmin = gmin_val;
            match self.solve_in_place(topo, backend, linear_solver, &mut x) {
                Ok((it, r)) => {
                    total_iters += it;
                    last_res = r;
                }
                Err(e) => {
                    topo.soa.gmin = orig_gmin;
                    return Err(e);
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

    fn solve_with_source_stepping(
        &mut self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
    ) -> SimResult<DcOpResult> {
        let orig_v: Vec<f64> = topo
            .soa
            .vsrc_waveforms
            .iter()
            .map(|w| w.dc_value())
            .collect();
        let orig_i: Vec<f64> = topo
            .soa
            .isrc_waveforms
            .iter()
            .map(|w| w.dc_value())
            .collect();

        let dim = topo.soa.mna_dim;
        let mut x = vec![0.0_f64; dim];
        Self::sync_device_voltages(topo, &x);

        let mut total_iters = 0usize;
        let mut last_res = 0.0;
        let steps = 10;

        for s in 1..=steps {
            let scale = (s as f64) / (steps as f64);
            for (idx, &v) in orig_v.iter().enumerate() {
                topo.soa.vsrc_values[idx] = v * scale;
            }
            for (idx, &c) in orig_i.iter().enumerate() {
                topo.soa.isrc_values[idx] = c * scale;
            }
            let (it, r) = self.solve_in_place(topo, backend, linear_solver, &mut x)?;
            total_iters += it;
            last_res = r;
        }

        Ok(DcOpResult {
            solution: x,
            iterations: total_iters,
            max_residual: last_res,
        })
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
        if step == 0.0 {
            return Err(SimError::TopologyError(
                "DC sweep step cannot be zero".to_string(),
            ));
        }
        topo.soa.is_transient = false;
        topo.soa.update_sources_at_time(None);

        let dim = topo.soa.mna_dim;
        self.ensure_dim(dim);

        let mut sweep_values = Vec::new();
        let mut cur = start;
        let dir = if stop >= start { 1.0 } else { -1.0 };
        let step_signed = step.abs() * dir;
        while (dir > 0.0 && cur <= stop + 1e-12 * step.abs())
            || (dir < 0.0 && cur >= stop - 1e-12 * step.abs())
        {
            sweep_values.push(cur);
            cur += step_signed;
        }

        let mut solutions = Vec::with_capacity(sweep_values.len());
        let mut x = vec![0.0_f64; dim];
        let mut prev_x = vec![0.0_f64; dim];
        let mut prev2_x = vec![0.0_f64; dim];
        let mut total_iterations = 0usize;

        let is_linear = topo.soa.is_linear();

        for (step_idx, &val) in sweep_values.iter().enumerate() {
            topo.soa.set_source_dc_value(source_name, val)?;

            if is_linear && step_idx > 0 && linear_solver.has_valid_factorization() {
                // Linear DC Sweep Fast-Path: Jacobian is invariant, only RHS source changed!
                backend.evaluate_devices(&mut topo.soa, &x)?;
                backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut self.rhs)?;
                linear_solver.solve_cached(&self.rhs, &mut x)?;
                total_iterations += 1;
                solutions.push(x.clone());
                continue;
            }

            // Secant Trajectory Predictor for nonlinear sweeps
            if step_idx >= 2 {
                for i in 0..dim {
                    x[i] = 2.0_f64.mul_add(prev_x[i], -prev2_x[i]);
                }
                Self::sync_device_voltages(topo, &x);
            }

            let (iters, _) = self.solve_in_place(topo, backend, linear_solver, &mut x)?;
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
    }
}
