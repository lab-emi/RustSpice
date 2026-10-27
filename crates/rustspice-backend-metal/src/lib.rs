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
    fn eval_diodes_df64(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) {
        let n_diodes = state.diode_names.len();
        if n_diodes == 0 {
            return;
        }
        let vt_df = Df64::from_f64(state.temp_vt);
        let gmin_df = Df64::from_f64(state.gmin);
        let sqrt2 = std::f64::consts::SQRT_2;

        for i in 0..n_diodes {
            let va = DeviceSoaBuffer::node_voltage(voltages, state.diode_anode[i]);
            let vc = DeviceSoaBuffer::node_voltage(voltages, state.diode_cathode[i]);
            let vd_raw = va - vc;
            let n_vt = state.diode_n[i] * state.temp_vt;
            let vcrit = n_vt * (n_vt / (sqrt2 * state.diode_is[i])).ln();
            let vd_lim = pnjlim(vd_raw, state.diode_vd[i], n_vt, vcrit);
            state.diode_vd[i] = vd_lim;

            // Execute diode equations in Df64 double-float precision
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

            state.cond_buf[state.diode_cond_buf_idx[i] as usize] = state.diode_gd[i];
            state.rhs_buf[state.diode_rhs_buf_idx[i] as usize] = state.diode_ieq[i];
        }
    }

    /// Evaluates MOSFETs in `Df64` double-float arithmetic on UMA shared buffers.
    fn eval_mosfets_df64(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) {
        let n_mos = state.mos_names.len();
        if n_mos == 0 {
            return;
        }
        let half = Df64::from_f64(0.5);

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
            let kp = Df64::from_f64(state.mos_kp[i]);
            let w = Df64::from_f64(state.mos_w[i]);
            let l = Df64::from_f64(state.mos_l[i].max(1e-12));
            let beta = kp * (w / l);
            let von = Df64::from_f64(pol * state.mos_vto[i]);
            let lambda = Df64::from_f64(state.mos_lambda[i]);
            let clm = Df64::ONE + lambda * vds;

            let v_ov = vgs - von;
            let (cdrain, gm, gds) = if v_ov.to_f64() <= 0.0 {
                (Df64::ZERO, Df64::ZERO, Df64::ZERO)
            } else if vds.to_f64() < v_ov.to_f64() {
                let core = vds * (v_ov - half * vds);
                let cd = beta * core * clm;
                let gm_v = beta * vds * clm;
                let gds_v = beta * (v_ov - vds) * clm + beta * lambda * core;
                (cd, gm_v, gds_v)
            } else {
                let core = half * v_ov * v_ov;
                let cd = beta * core * clm;
                let gm_v = beta * v_ov * clm;
                let gds_v = beta * lambda * core;
                (cd, gm_v, gds_v)
            };

            let id_oriented = pol * mode * cdrain.to_f64() + state.gmin * (vd - vs);
            let gm_f = gm.to_f64();
            let gds_f = gds.to_f64() + state.gmin;

            state.mos_id[i] = id_oriented;
            state.mos_gm[i] = gm_f;
            state.mos_gds[i] = gds_f;
            state.mos_gmbs[i] = 0.0;

            if mode > 0.0 {
                let ieq = id_oriented - gm_f * (vg - vs) - gds_f * (vd - vs);
                state.mos_ieq[i] = ieq;
                state.cond_buf[state.mos_gm_buf_idx[i] as usize] = gm_f;
                state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds_f;
                state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = 0.0;
                state.rhs_buf[state.mos_rhs_buf_idx[i] as usize] = ieq;
            } else {
                let ieq = id_oriented + gm_f * (vg - vd) - gds_f * (vd - vs);
                state.mos_ieq[i] = ieq;
                state.cond_buf[state.mos_gm_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds_f;
                state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = 0.0;
                state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = -gm_f;
                state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = 0.0;
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

        self.eval_diodes_df64(state, voltages);
        self.eval_mosfets_df64(state, voltages);
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
        } else {
            state.gather_table.gather_into(
                &state.cond_buf,
                &state.rhs_buf,
                &mut jacobian.values,
                rhs,
            );
            jacobian.add_diagonal_gmin(state.num_nodes, state.gmin);
        }
        Ok(())
    }
}
