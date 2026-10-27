//! Discrete Adjoint Sensitivity Analysis Engine ($\frac{\mathrm{d}\mathcal{L}}{\mathrm{d}\theta}$).
//!
//! Computes exact analytical gradients of any scalar circuit objective $\mathcal{L}(x^*)$
//! with respect to an arbitrary number of device parameters $P$ in **a single transposed
//! sparse triangular solve**:
//! $$J^T \lambda = -\left(\frac{\partial \mathcal{L}}{\partial x^*}\right)^T, \quad
//!   \frac{\mathrm{d}\mathcal{L}}{\mathrm{d}\theta_p} = \frac{\partial \mathcal{L}}{\partial \theta_p} + \lambda^T \frac{\partial F}{\partial \theta_p}$$

use rustspice_core::{CompiledTopology, ComputeBackend, DeviceSoaBuffer, SimError, SimResult};
use serde::{Deserialize, Serialize};

use crate::linear::AdaptiveSparseSolver;

/// Specifies a device parameter $\theta_p$ for gradient extraction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGradSpec {
    pub device_name: String,
    pub param_name: String,
}

impl ParameterGradSpec {
    pub fn new(device_name: impl Into<String>, param_name: impl Into<String>) -> Self {
        Self {
            device_name: device_name.into(),
            param_name: param_name.into(),
        }
    }
}

/// Complete output of a Discrete Adjoint Sensitivity calculation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdjointGradientResult {
    /// Adjoint state vector $\lambda$ of length `mna_dim`.
    pub adjoint_lambda: Vec<f64>,
    /// Computed total derivatives $\frac{\mathrm{d}\mathcal{L}}{\mathrm{d}\theta_p}$ in the order of `specs`.
    pub gradients: Vec<f64>,
}

/// Discrete Adjoint Sensitivity Engine.
#[derive(Clone, Debug, Default)]
pub struct AdjointSensitivityEngine;

impl AdjointSensitivityEngine {
    pub fn new() -> Self {
        Self
    }

    #[inline]
    fn node_adj(lambda: &[f64], node_id: usize) -> f64 {
        if node_id == 0 {
            0.0
        } else {
            lambda[node_id - 1]
        }
    }

    /// Computes exact Discrete Adjoint gradients $\frac{\mathrm{d}\mathcal{L}}{\mathrm{d}\theta}$
    /// at the converged operating point `x_star` given the objective gradient
    /// `d_loss_d_x` $=\left(\frac{\partial \mathcal{L}}{\partial x^*}\right)^T$.
    pub fn compute_dc_adjoint_gradients(
        &self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        x_star: &[f64],
        d_loss_d_x: &[f64],
        specs: &[ParameterGradSpec],
    ) -> SimResult<AdjointGradientResult> {
        let dim = topo.soa.mna_dim;
        if x_star.len() != dim || d_loss_d_x.len() != dim {
            return Err(SimError::TopologyError(
                "Dimension mismatch in adjoint sensitivity inputs".to_string(),
            ));
        }

        // Ensure device derivatives and Jacobian are evaluated at the exact converged state x_star
        let mut rhs_dummy = vec![0.0_f64; dim];
        let mut x_dummy = vec![0.0_f64; dim];
        backend.evaluate_devices(&mut topo.soa, x_star)?;
        backend.stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut rhs_dummy)?;

        // Ensure LU factorization is up to date at x_star
        linear_solver.factorize_and_solve(&topo.jacobian, &rhs_dummy, &mut x_dummy)?;

        // Solve J^T * lambda = - (dL / dx)^T
        let neg_dl_dx: Vec<f64> = d_loss_d_x.iter().map(|&g| -g).collect();
        let mut lambda = vec![0.0_f64; dim];
        linear_solver.solve_transpose_cached(&topo.jacobian, &neg_dl_dx, &mut lambda)?;

        // Contract lambda^T * (dF / d_theta) for every requested parameter in O(1) per parameter!
        let mut gradients = Vec::with_capacity(specs.len());
        for spec in specs {
            let grad = self.contract_parameter_derivative(&topo.soa, x_star, &lambda, spec)?;
            gradients.push(grad);
        }

        Ok(AdjointGradientResult {
            adjoint_lambda: lambda,
            gradients,
        })
    }

    /// Extracts all resistor, diode, and MOSFET parameter sensitivities simultaneously in one pass.
    pub fn compute_all_resistor_gradients(
        &self,
        topo: &mut CompiledTopology,
        backend: &dyn ComputeBackend,
        linear_solver: &mut AdaptiveSparseSolver,
        x_star: &[f64],
        target_node_id: usize,
    ) -> SimResult<AdjointGradientResult> {
        let dim = topo.soa.mna_dim;
        let mut dl_dx = vec![0.0_f64; dim];
        if target_node_id > 0 && target_node_id - 1 < dim {
            dl_dx[target_node_id - 1] = 1.0;
        }
        let specs: Vec<ParameterGradSpec> = topo
            .soa
            .res_names
            .iter()
            .map(|name| ParameterGradSpec::new(name.clone(), "r"))
            .collect();
        self.compute_dc_adjoint_gradients(topo, backend, linear_solver, x_star, &dl_dx, &specs)
    }

    fn contract_parameter_derivative(
        &self,
        soa: &DeviceSoaBuffer,
        x_star: &[f64],
        lambda: &[f64],
        spec: &ParameterGradSpec,
    ) -> SimResult<f64> {
        let dev = &spec.device_name;
        let p_lower = spec.param_name.to_ascii_lowercase();

        // 1. Resistor parameter `R`
        for (i, name) in soa.res_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(dev) && (p_lower == "r" || p_lower == "resistance") {
                let vp = DeviceSoaBuffer::node_voltage(x_star, soa.res_n_pos[i]);
                let vn = DeviceSoaBuffer::node_voltage(x_star, soa.res_n_neg[i]);
                let lp = Self::node_adj(lambda, soa.res_n_pos[i]);
                let ln = Self::node_adj(lambda, soa.res_n_neg[i]);
                let r = soa.res_resistance[i];
                // dF / dR = -(vp - vn) / R^2 at n_pos, +(vp - vn) / R^2 at n_neg
                let df_dr = -(vp - vn) / (r * r);
                return Ok((lp - ln) * df_dr);
            }
        }

        // 2. MOSFET parameters `W`, `L`, `VTO`, `KP`
        for (i, name) in soa.mos_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(dev) {
                let ld = Self::node_adj(lambda, soa.mos_drain[i]);
                let ls = Self::node_adj(lambda, soa.mos_source[i]);
                let did_dtheta = match p_lower.as_str() {
                    "w" => soa.mos_did_dw[i],
                    "l" => soa.mos_did_dl[i],
                    "vto" | "vt0" => soa.mos_did_dvto[i],
                    "kp" => soa.mos_did_dkp[i],
                    _ => {
                        return Err(SimError::TopologyError(format!(
                            "Unsupported MOSFET sensitivity parameter `{}`",
                            spec.param_name
                        )))
                    }
                };
                return Ok((ld - ls) * did_dtheta);
            }
        }

        // 3. Diode parameters `IS`, `N`, `AREA`
        for (i, name) in soa.diode_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(dev) {
                let la = Self::node_adj(lambda, soa.diode_anode[i]);
                let lc = Self::node_adj(lambda, soa.diode_cathode[i]);
                let did_dtheta = match p_lower.as_str() {
                    "is" => soa.diode_did_dis[i] * soa.diode_area[i],
                    "n" => soa.diode_did_dn[i],
                    "area" => {
                        let base_is = soa.diode_is[i] / soa.diode_area[i].max(1e-30);
                        soa.diode_did_dis[i] * base_is
                    }
                    _ => {
                        return Err(SimError::TopologyError(format!(
                            "Unsupported Diode sensitivity parameter `{}`",
                            spec.param_name
                        )))
                    }
                };
                return Ok((la - lc) * did_dtheta);
            }
        }

        // 4. Independent Voltage Source `DC` / `V`
        for (i, name) in soa.vsrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(dev) {
                let br = soa.vsrc_branch_idx[i];
                return Ok(-lambda[br]);
            }
        }

        // 5. Independent Current Source `DC` / `I`
        for (i, name) in soa.isrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(dev) {
                let lp = Self::node_adj(lambda, soa.isrc_n_pos[i]);
                let ln = Self::node_adj(lambda, soa.isrc_n_neg[i]);
                return Ok(lp - ln);
            }
        }

        Err(SimError::TopologyError(format!(
            "Parameter `{}.{}` not found for adjoint differentiation",
            spec.device_name, spec.param_name
        )))
    }
}
