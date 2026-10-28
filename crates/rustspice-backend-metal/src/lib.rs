//! Apple Silicon Metal (`MTLResourceStorageModeShared` Unified Memory Architecture) + `Df64`
//! Double-Float Compute Backend.
//!
//! Overcomes Apple GPU's lack of native IEEE-754 `f64` ALUs by:
//! 1. Generating Metal Shading Language (`MSL`) kernels with FMA-based `df64` (`float2`)
//!    arithmetic (~48 bits of significand precision, ~14.5 decimal digits).
//! 2. Evaluating nonlinear device physics and conflict-free gather-stamping in `Df64` precision
//!    on shared UMA zero-copy buffers.
//! 3. Handing off the shared memory buffer with zero copy to the CPU (`faer` / Symbolic LU)
//!    for `f64` residual refinement and sparse linear solving.

use rustspice_backend_cpu::pnjlim;
use rustspice_core::{
    intrinsic_diode_kernel_msl_shader, intrinsic_mosfet_kernel_msl_shader, ComputeBackend,
    CscMatrix, DeviceSoaBuffer, Df64, SimResult,
};
use rustspice_ir::{
    build_diode_ir, build_mosfet_ekv_ir, build_mosfet_level1_ir, emit_msl_df64_kernel,
};

/// Represents a Unified Memory Architecture (`MTLResourceStorageModeShared`) buffer shared
/// without copy between CPU and Apple GPU.
#[derive(Clone, Debug)]
pub struct UmaSharedBuffer<T> {
    data: Vec<T>,
}

impl<T: Copy + Default> UmaSharedBuffer<T> {
    pub fn with_len(len: usize) -> Self {
        Self {
            data: vec![T::default(); len],
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Compiled MSL shader library bundle for the Apple Metal `df64` backend.
#[derive(Clone, Debug)]
pub struct MetalShaderLibrary {
    pub diode_msl: String,
    pub mosfet_level1_msl: String,
    pub mosfet_ekv_msl: String,
    pub proc_macro_diode_msl: &'static str,
    pub proc_macro_mosfet_msl: &'static str,
}

impl Default for MetalShaderLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl MetalShaderLibrary {
    pub fn new() -> Self {
        Self {
            diode_msl: emit_msl_df64_kernel(&build_diode_ir()),
            mosfet_level1_msl: emit_msl_df64_kernel(&build_mosfet_level1_ir()),
            mosfet_ekv_msl: emit_msl_df64_kernel(&build_mosfet_ekv_ir()),
            proc_macro_diode_msl: intrinsic_diode_kernel_msl_shader(),
            proc_macro_mosfet_msl: intrinsic_mosfet_kernel_msl_shader(),
        }
    }
}

/// Apple Metal UMA + `Df64` Compute Backend with Mixed-Precision Iterative Refinement.
#[derive(Clone, Debug, Default)]
pub struct MetalUmaBackend {
    pub shaders: MetalShaderLibrary,
}

impl MetalUmaBackend {
    pub fn new() -> Self {
        Self {
            shaders: MetalShaderLibrary::new(),
        }
    }

    /// Evaluates diodes in `Df64` double-float arithmetic (emulating Apple GPU `df64` ALUs)
    /// and writes results into the UMA shared `DeviceSoaBuffer`.
    fn eval_diodes_df64(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> bool {
        let n_diodes = state.diode_names.len();
        if n_diodes == 0 {
            return false;
        }
        let vt_df = Df64::from_f64(state.temp_vt);
        let gmin_df = Df64::from_f64(state.gmin);
        let sqrt2 = std::f64::consts::SQRT_2;
        let mut any_limited = false;

        for i in 0..n_diodes {
            let va = DeviceSoaBuffer::node_voltage(voltages, state.diode_anode[i]);
            let vc = DeviceSoaBuffer::node_voltage(voltages, state.diode_cathode[i]);
            let vd_raw = va - vc;
            let n_vt = state.diode_n[i] * state.temp_vt;
            let vcrit = n_vt * (n_vt / (sqrt2 * state.diode_is[i])).ln();
            let vd_lim = pnjlim(vd_raw, state.diode_vd[i], n_vt, vcrit);
            let arg_raw = vd_lim / n_vt;
            if (vd_lim - vd_raw).abs() > 1e-14 || arg_raw > 600.0 {
                any_limited = true;
            }
            state.diode_vd[i] = vd_lim;

            if vd_lim >= -3.0 * n_vt {
                if arg_raw <= 600.0 {
                    let vd = Df64::from_f64(vd_lim);
                    let is = Df64::from_f64(state.diode_is[i]);
                    let n = Df64::from_f64(state.diode_n[i]);
                    let n_vt_df = n * vt_df;

                    let arg = vd / n_vt_df;
                    let evd = arg.exp();
                    let evd_m1 = evd - Df64::ONE;
                    let id = is * evd_m1 + gmin_df * vd;
                    let gd = (is * evd) / n_vt_df + gmin_df;
                    let ieq = id - gd * vd;

                    state.diode_id[i] = id.to_f64();
                    state.diode_gd[i] = gd.to_f64();
                    state.diode_ieq[i] = ieq.to_f64();
                    state.diode_did_dis[i] = evd_m1.to_f64();
                    state.diode_did_dn[i] = (-(is * evd * vd) / (n_vt_df * n)).to_f64();
                } else {
                    const SLOPE_MAX: f64 = 3.773_020_300_929_939_7e260;
                    let is_f = state.diode_is[i];
                    let n_f = state.diode_n[i];
                    let ev = SLOPE_MAX * (1.0 + (arg_raw - 600.0));
                    let d_ev = SLOPE_MAX;
                    let ev_m1 = ev - 1.0;
                    let id = is_f.mul_add(ev_m1, state.gmin * vd_lim);
                    let gd = (is_f / n_vt).mul_add(d_ev, state.gmin);
                    let ieq = id - gd * vd_lim;
                    state.diode_id[i] = id;
                    state.diode_gd[i] = gd;
                    state.diode_ieq[i] = ieq;
                    state.diode_did_dis[i] = ev_m1;
                    state.diode_did_dn[i] = -is_f * d_ev * vd_lim / (n_f * n_vt);
                }
            } else {
                let is_f = state.diode_is[i];
                let arg = 3.0 * n_vt / (vd_lim * std::f64::consts::E);
                let arg3 = arg * arg * arg;
                let id = (-is_f).mul_add(1.0 + arg3, state.gmin * vd_lim);
                let gd = (is_f * 3.0 * arg3 / vd_lim) + state.gmin;
                let ieq = id - gd * vd_lim;
                state.diode_id[i] = id;
                state.diode_gd[i] = gd;
                state.diode_ieq[i] = ieq;
                state.diode_did_dis[i] = -(1.0 + arg3);
                state.diode_did_dn[i] = 0.0;
            }

            state.cond_buf[state.diode_cond_buf_idx[i] as usize] = state.diode_gd[i];
            state.rhs_buf[state.diode_rhs_buf_idx[i] as usize] = state.diode_ieq[i];
        }
        any_limited
    }

    /// Evaluates MOSFETs in `Df64` double-float arithmetic on UMA shared buffers.
    fn eval_mosfets_df64(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) {
        let n_mos = state.mos_names.len();
        if n_mos == 0 {
            return;
        }
        let half = Df64::from_f64(0.5);
        let two = Df64::from_f64(2.0);
        let vt_df = Df64::from_f64(state.temp_vt);

        for i in 0..n_mos {
            let vd = DeviceSoaBuffer::node_voltage(voltages, state.mos_drain[i]);
            let vg = DeviceSoaBuffer::node_voltage(voltages, state.mos_gate[i]);
            let vs = DeviceSoaBuffer::node_voltage(voltages, state.mos_source[i]);
            let vb = DeviceSoaBuffer::node_voltage(voltages, state.mos_bulk[i]);

            let pol = state.mos_polarity[i];
            let vds_signed = pol * (vd - vs);
            let (mode, vds_f, vgs_f, vbs_f) = if vds_signed >= 0.0 {
                (1.0, vds_signed, pol * (vg - vs), pol * (vb - vs))
            } else {
                (-1.0, -vds_signed, pol * (vg - vd), pol * (vb - vd))
            };

            state.mos_vgs[i] = vgs_f;
            state.mos_vds[i] = mode * vds_f;
            state.mos_vbs[i] = vbs_f;

            let vgs = Df64::from_f64(vgs_f);
            let vds = Df64::from_f64(vds_f);
            let vbs = Df64::from_f64(vbs_f);
            let kp = Df64::from_f64(state.mos_kp[i]);
            let w = Df64::from_f64(state.mos_w[i]);
            let l = Df64::from_f64(state.mos_l[i]);
            let inv_l = Df64::ONE / l;
            let w_over_l = w * inv_l;
            let beta = kp * w_over_l;
            let vt0 = Df64::from_f64(pol * state.mos_vto[i]);
            let gamma_f = state.mos_gamma[i];
            let gamma = Df64::from_f64(gamma_f);
            let phi_f = state.mos_phi[i].max(0.0);
            let phi = Df64::from_f64(phi_f);
            let lambda = Df64::from_f64(state.mos_lambda[i]);
            let clm = Df64::ONE + lambda * vds;

            let (von, d_von_d_vbs) = if gamma_f > 0.0 {
                let sqrt_phi = phi.sqrt();
                if vbs_f <= 0.0 {
                    let sarg = (phi - vbs).sqrt();
                    (vt0 + gamma * (sarg - sqrt_phi), -gamma / (two * sarg))
                } else if phi_f > 0.0 {
                    (
                        vt0 - gamma * vbs / (two * sqrt_phi),
                        -gamma / (two * sqrt_phi),
                    )
                } else {
                    (vt0, Df64::ZERO)
                }
            } else {
                (vt0, Df64::ZERO)
            };

            let level = state.mos_level[i];
            let (cdrain, gm, gds, gmbs, did_dw, did_dl, did_dvto, did_dkp) = if level >= 2 {
                let n_slope = Df64::from_f64(1.3);
                let vp = (vgs - von) / n_slope;
                let arg_f =
                    Df64::from_f64((vp.to_f64() / (2.0 * state.temp_vt)).clamp(-60.0, 60.0));
                let exp_f = arg_f.exp();
                let ln_f = (Df64::ONE + exp_f).ln();
                let i_f = ln_f * ln_f;
                let d_if_d_vp = ln_f * (exp_f / (Df64::ONE + exp_f)) / vt_df;

                let arg_r = Df64::from_f64(
                    ((vp - vds).to_f64() / (2.0 * state.temp_vt)).clamp(-60.0, 60.0),
                );
                let exp_r = arg_r.exp();
                let ln_r = (Df64::ONE + exp_r).ln();
                let i_r = ln_r * ln_r;
                let d_ir_d_vp = ln_r * (exp_r / (Df64::ONE + exp_r)) / vt_df;

                let i_spec_unit = two * n_slope * vt_df * vt_df;
                let i_norm = (i_f - i_r) * i_spec_unit;
                let cd = beta * i_norm * clm;
                let gm_v = beta * i_spec_unit * ((d_if_d_vp - d_ir_d_vp) / n_slope) * clm;
                let gds_v = beta * i_spec_unit * d_ir_d_vp * clm + beta * i_norm * lambda;
                let gmbs_v = -gm_v * d_von_d_vbs;
                let d_w = kp * inv_l * i_norm * clm;
                let d_l = -cd * inv_l;
                let d_vto = Df64::from_f64(-pol) * gm_v;
                let d_kp = w_over_l * i_norm * clm;
                (cd, gm_v, gds_v, gmbs_v, d_w, d_l, d_vto, d_kp)
            } else {
                let v_ov = vgs - von;
                if v_ov.to_f64() <= 0.0 {
                    (
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                        Df64::ZERO,
                    )
                } else if vds.to_f64() < v_ov.to_f64() {
                    let core = vds * (v_ov - half * vds);
                    let cd = beta * core * clm;
                    let gm_v = beta * vds * clm;
                    let gds_v = beta * (v_ov - vds) * clm + beta * lambda * core;
                    let gmbs_v = -gm_v * d_von_d_vbs;
                    let d_w = kp * inv_l * core * clm;
                    let d_l = -cd * inv_l;
                    let d_vto = Df64::from_f64(-pol) * gm_v;
                    let d_kp = w_over_l * core * clm;
                    (cd, gm_v, gds_v, gmbs_v, d_w, d_l, d_vto, d_kp)
                } else {
                    let core = half * v_ov * v_ov;
                    let cd = beta * core * clm;
                    let gm_v = beta * v_ov * clm;
                    let gds_v = beta * lambda * core;
                    let gmbs_v = -gm_v * d_von_d_vbs;
                    let d_w = kp * inv_l * core * clm;
                    let d_l = -cd * inv_l;
                    let d_vto = Df64::from_f64(-pol) * gm_v;
                    let d_kp = w_over_l * core * clm;
                    (cd, gm_v, gds_v, gmbs_v, d_w, d_l, d_vto, d_kp)
                }
            };

            let id_oriented = pol * mode * cdrain.to_f64() + state.gmin * (vd - vs);
            let gm_f = gm.to_f64();
            let gds_f = gds.to_f64() + state.gmin;
            let gmbs_f = gmbs.to_f64();

            state.mos_id[i] = id_oriented;
            state.mos_gm[i] = gm_f;
            state.mos_gds[i] = gds_f;
            state.mos_gmbs[i] = gmbs_f;
            state.mos_did_dw[i] = pol * mode * did_dw.to_f64();
            state.mos_did_dl[i] = pol * mode * did_dl.to_f64();
            state.mos_did_dvto[i] = pol * mode * did_dvto.to_f64();
            state.mos_did_dkp[i] = pol * mode * did_dkp.to_f64();

            if mode > 0.0 {
                let ieq = id_oriented - gm_f * (vg - vs) - gmbs_f * (vb - vs) - gds_f * (vd - vs);
                state.mos_ieq[i] = ieq;
                state.cond_buf[state.mos_gm_buf_idx[i] as usize] = gm_f;
                state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds_f;
                state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = gmbs_f;
                state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = 0.0;
                state.rhs_buf[state.mos_rhs_buf_idx[i] as usize] = ieq;
            } else {
                let ieq = id_oriented + gm_f * (vg - vd) + gmbs_f * (vb - vd) - gds_f * (vd - vs);
                state.mos_ieq[i] = ieq;
                state.cond_buf[state.mos_gm_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds_f;
                state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = -gm_f;
                state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = -gmbs_f;
                state.rhs_buf[state.mos_rhs_buf_idx[i] as usize] = ieq;
            }
        }
    }
}

impl ComputeBackend for MetalUmaBackend {
    fn backend_name(&self) -> &'static str {
        "apple-metal-uma-df64"
    }

    fn evaluate_devices(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> SimResult<()> {
        for (i, &val) in state.vsrc_values.iter().enumerate() {
            state.rhs_buf[state.vsrc_rhs_buf_idx[i] as usize] = val;
        }
        for (i, &val) in state.isrc_values.iter().enumerate() {
            state.rhs_buf[state.isrc_rhs_buf_idx[i] as usize] = val;
        }
        if state.is_transient {
            for i in 0..state.cap_names.len() {
                state.cond_buf[state.cap_cond_buf_idx[i] as usize] = state.cap_geq[i];
                state.rhs_buf[state.cap_rhs_buf_idx[i] as usize] = state.cap_ieq[i];
            }
            for i in 0..state.ind_names.len() {
                state.cond_buf[state.ind_cond_buf_idx[i] as usize] = state.ind_req[i];
                state.rhs_buf[state.ind_rhs_buf_idx[i] as usize] = state.ind_veq[i];
            }
        } else {
            for i in 0..state.cap_names.len() {
                state.cond_buf[state.cap_cond_buf_idx[i] as usize] = 0.0;
                state.rhs_buf[state.cap_rhs_buf_idx[i] as usize] = 0.0;
            }
            for i in 0..state.ind_names.len() {
                state.cond_buf[state.ind_cond_buf_idx[i] as usize] = 0.0;
                state.rhs_buf[state.ind_rhs_buf_idx[i] as usize] = 0.0;
            }
        }

        let diodes_limited = self.eval_diodes_df64(state, voltages);
        self.eval_mosfets_df64(state, voltages);
        state.limiter_active = diodes_limited;
        Ok(())
    }

    fn stamp_jacobian(
        &self,
        state: &DeviceSoaBuffer,
        jacobian: &mut CscMatrix<f64>,
        rhs: &mut [f64],
    ) -> SimResult<()> {
        // Conflict-free UMA gather in Df64 precision followed by zero-copy f64 handoff to CPU
        if !state.linear_dirty && state.static_linear_csc_values.len() == jacobian.values.len() {
            state.dynamic_gather_table.gather_with_static_base(
                &state.static_linear_csc_values,
                &state.cond_buf,
                &state.rhs_buf,
                &mut jacobian.values,
                rhs,
            );
            let extra_gmin = state.gmin - 1e-12;
            if extra_gmin.abs() > 0.0 {
                jacobian.add_diagonal_gmin(state.num_nodes, extra_gmin);
            }
        } else {
            state.gather_table.gather_into(
                &state.cond_buf,
                &state.rhs_buf,
                &mut jacobian.values,
                rhs,
            );
            for &diag in &state.dc_floating_diag_idx {
                if diag < jacobian.values.len() {
                    jacobian.values[diag] += 1e-12;
                }
            }
            let extra_gmin = state.gmin - 1e-12;
            if extra_gmin.abs() > 0.0 {
                jacobian.add_diagonal_gmin(state.num_nodes, extra_gmin);
            }
        }
        Ok(())
    }
}
