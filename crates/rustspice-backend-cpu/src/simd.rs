//! Hardware SIMD (`pulp` AVX-512 / AVX2 / NEON) SoA device evaluator and conflict-free
//! Jacobian stamper.

use pulp::Arch;
use rustspice_core::{ComputeBackend, CscMatrix, DeviceSoaBuffer, SimResult};

/// SPICE3f5 / `ngspice` PN-junction logarithmic voltage step limiter (`pnjlim`).
///
/// Prevents exponential numerical overflow in Newton-Raphson iterations when a PN junction
/// transitions abruptly into forward bias.
#[inline]
pub fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > (vt + vt) {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 {
                vold + vt * arg.ln()
            } else {
                vcrit
            }
        } else {
            vt * (vnew / vt).ln()
        }
    } else {
        vnew
    }
}

/// SPICE3f5 / `ngspice` MOSFET gate-to-channel voltage step limiter (`fetlim`).
#[inline]
pub fn fetlim(vnew: f64, vold: f64, vto: f64) -> f64 {
    let vtsthi = (2.0 * (vold - vto)).abs() + 2.0;
    let vtstlo = vtsthi * 0.5 + 2.0;
    let vtox = vto + 3.5;
    let delv = vnew - vold;

    if vold >= vto {
        if vold >= vtox {
            if delv <= 0.0 {
                if vnew >= vtox {
                    if -delv > vtstlo {
                        vold - vtstlo
                    } else {
                        vnew
                    }
                } else {
                    vnew.max(vto + 2.0)
                }
            } else if delv >= vtsthi {
                vold + vtsthi
            } else {
                vnew
            }
        } else if delv <= 0.0 {
            vnew.max(vto - 0.5)
        } else {
            vnew.min(vto + 4.0)
        }
    } else if delv <= 0.0 {
        if -delv > vtsthi {
            vold - vtsthi
        } else {
            vnew
        }
    } else {
        let vtemp = vto + 0.5;
        if vnew <= vtemp {
            if delv > vtstlo {
                vold + vtstlo
            } else {
                vnew
            }
        } else {
            vtemp
        }
    }
}

/// Maximum Shockley diode exponent argument $V_D / (N V_T)$ evaluated with exact `f64::exp()`.
///
/// IEEE-754 `f64::exp(x)` overflows at $x \approx 709.78$. Choosing `600.0` ($\exp(600) \approx
/// 3.773\times 10^{260}$) keeps every diode up to $\sim 15.5\text{ V}$ forward bias (at $N=1$,
/// $T=300.15\text{ K}$) on the exact exponential curve while leaving $\sim 50$ decades of headroom
/// below `f64::MAX` for $I_S \exp(\cdot)$ and $g_d = (I_S / N V_T) \exp(\cdot)$ during intermediate
/// Newton steps. Above `DIODE_EXP_ARG_MAX`, a $C^1$ linear continuation $\exp(x_{\max})(1 + x - x_{\max})$
/// prevents `NaN`/`Inf` in the Jacobian; the nonlinear solver rejects any converged point that
/// still lies in the linear-continuation region (`SimError::DeviceRangeExceeded`).
pub const DIODE_EXP_ARG_MAX: f64 = 600.0;
const DIODE_EXP_SLOPE_MAX: f64 = 3.773_020_300_929_939_7e260; // 600.0_f64.exp()

#[inline]
pub fn diode_exp(arg: f64) -> (f64, f64, bool) {
    if arg <= DIODE_EXP_ARG_MAX {
        let e = arg.exp();
        (e, e, false)
    } else {
        let excess = arg - DIODE_EXP_ARG_MAX;
        (
            DIODE_EXP_SLOPE_MAX * (1.0 + excess),
            DIODE_EXP_SLOPE_MAX,
            true,
        )
    }
}

/// High-throughput CPU backend utilizing `pulp` SIMD runtime dispatch (`AVX-512` / `AVX2` / `NEON`)
/// and `rayon` multi-threading for large device arrays.
#[derive(Clone, Debug)]
pub struct CpuSimdBackend {
    arch: Arch,
    pub use_step_limiting: bool,
}

impl Default for CpuSimdBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuSimdBackend {
    /// Creates a new [`CpuSimdBackend`] with runtime CPU SIMD feature detection.
    pub fn new() -> Self {
        Self {
            arch: Arch::new(),
            use_step_limiting: true,
        }
    }

    #[inline]
    fn eval_diodes(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> bool {
        let n_diodes = state.diode_names.len();
        if n_diodes == 0 {
            return false;
        }

        let vt_base = state.temp_vt;
        let gmin = state.gmin;
        let use_limit = self.use_step_limiting;
        let sqrt2 = std::f64::consts::SQRT_2;
        let mut any_limited = false;

        // Gather terminal voltage differences in a tight SIMD-friendly loop
        for i in 0..n_diodes {
            let va = DeviceSoaBuffer::node_voltage(voltages, state.diode_anode[i]);
            let vc = DeviceSoaBuffer::node_voltage(voltages, state.diode_cathode[i]);
            let vd_raw = va - vc;
            let n_vt = state.diode_n[i] * vt_base;
            let is = state.diode_is[i];
            let vcrit = n_vt * (n_vt / (sqrt2 * is)).ln();
            let vd = if use_limit {
                pnjlim(vd_raw, state.diode_vd[i], n_vt, vcrit)
            } else {
                vd_raw
            };
            let arg = vd / n_vt;
            if (vd - vd_raw).abs() > 1e-14 || arg > DIODE_EXP_ARG_MAX {
                any_limited = true;
            }
            state.diode_vd[i] = vd;
        }

        // Dispatch diode physics equation evaluation through pulp::Arch SIMD vectorization
        let vd_slice = state.diode_vd.as_slice();
        let is_slice = state.diode_is.as_slice();
        let n_slice = state.diode_n.as_slice();
        let id_slice = state.diode_id.as_mut_slice();
        let gd_slice = state.diode_gd.as_mut_slice();
        let ieq_slice = state.diode_ieq.as_mut_slice();
        let did_dis_slice = state.diode_did_dis.as_mut_slice();
        let did_dn_slice = state.diode_did_dn.as_mut_slice();

        self.arch.dispatch(|| {
            for i in 0..n_diodes {
                let vd = vd_slice[i];
                let is = is_slice[i];
                let n = n_slice[i];
                let n_vt = n * vt_base;
                let inv_n_vt = 1.0 / n_vt;

                let (id, gd, evd_m1, d_evd) = if vd >= -3.0 * n_vt {
                    let arg = vd * inv_n_vt;
                    let (ev, d_ev, _) = diode_exp(arg);
                    let ev_m1 = ev - 1.0;
                    let cur = is.mul_add(ev_m1, gmin * vd);
                    let cond = (is * inv_n_vt).mul_add(d_ev, gmin);
                    (cur, cond, ev_m1, d_ev)
                } else {
                    let arg = 3.0 * n_vt / (vd * std::f64::consts::E);
                    let arg3 = arg * arg * arg;
                    let cur = (-is).mul_add(1.0 + arg3, gmin * vd);
                    let cond = (is * 3.0 * arg3 / vd) + gmin;
                    (cur, cond, -(1.0 + arg3), 0.0)
                };

                id_slice[i] = id;
                gd_slice[i] = gd;
                ieq_slice[i] = id - gd * vd;
                did_dis_slice[i] = evd_m1;
                did_dn_slice[i] = -is * d_evd * vd * inv_n_vt / n;
            }
        });

        for i in 0..n_diodes {
            state.cond_buf[state.diode_cond_buf_idx[i] as usize] = state.diode_gd[i];
            state.rhs_buf[state.diode_rhs_buf_idx[i] as usize] = state.diode_ieq[i];
        }
        any_limited
    }

    #[inline]
    fn eval_mosfets(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) {
        let n_mos = state.mos_names.len();
        if n_mos == 0 {
            return;
        }
        let vt = state.temp_vt;
        let gmin = state.gmin;

        self.arch.dispatch(|| {
            for i in 0..n_mos {
                let vd = DeviceSoaBuffer::node_voltage(voltages, state.mos_drain[i]);
                let vg = DeviceSoaBuffer::node_voltage(voltages, state.mos_gate[i]);
                let vs = DeviceSoaBuffer::node_voltage(voltages, state.mos_source[i]);
                let vb = DeviceSoaBuffer::node_voltage(voltages, state.mos_bulk[i]);

                let pol = state.mos_polarity[i];
                let vds_signed = pol * (vd - vs);
                let (mode, vds, vgs, vbs) = if vds_signed >= 0.0 {
                    (1.0_f64, vds_signed, pol * (vg - vs), pol * (vb - vs))
                } else {
                    (-1.0_f64, -vds_signed, pol * (vg - vd), pol * (vb - vd))
                };

                state.mos_vgs[i] = vgs;
                state.mos_vds[i] = mode * vds;
                state.mos_vbs[i] = vbs;

                let kp = state.mos_kp[i];
                let w = state.mos_w[i];
                let l = state.mos_l[i];
                let inv_l = 1.0 / l;
                let w_over_l = w * inv_l;
                let beta = kp * w_over_l;
                let lambda = state.mos_lambda[i];
                let gamma = state.mos_gamma[i];
                let phi = state.mos_phi[i].max(0.0);
                // Threshold voltage in polarized coordinates (`pol * vto`, matching SPICE3f5)
                let vt0 = pol * state.mos_vto[i];

                let (von, d_von_d_vbs) = if gamma > 0.0 {
                    let sqrt_phi = phi.sqrt();
                    if vbs <= 0.0 {
                        let sarg = (phi - vbs).max(1e-30).sqrt();
                        (vt0 + gamma * (sarg - sqrt_phi), -gamma / (2.0 * sarg))
                    } else if sqrt_phi > 0.0 {
                        (
                            vt0 - gamma * vbs / (2.0 * sqrt_phi),
                            -gamma / (2.0 * sqrt_phi),
                        )
                    } else {
                        (vt0, 0.0)
                    }
                } else {
                    (vt0, 0.0)
                };

                let clm = lambda.mul_add(vds, 1.0);
                let level = state.mos_level[i];

                let (cdrain, gm, gds, gmbs, did_dw, did_dl, did_dvto, did_dkp) = if level >= 2 {
                    // Continuous all-region EKV model
                    let n_slope = 1.3;
                    let vp = (vgs - von) / n_slope;
                    let arg_f = (vp / (2.0 * vt)).clamp(-60.0, 60.0);
                    let exp_f = arg_f.exp();
                    let ln_f = (1.0 + exp_f).ln();
                    let i_f = ln_f * ln_f;
                    let d_if_d_vp = ln_f * (exp_f / (1.0 + exp_f)) / vt;

                    let arg_r = ((vp - vds) / (2.0 * vt)).clamp(-60.0, 60.0);
                    let exp_r = arg_r.exp();
                    let ln_r = (1.0 + exp_r).ln();
                    let i_r = ln_r * ln_r;
                    let d_ir_d_vp = ln_r * (exp_r / (1.0 + exp_r)) / vt;

                    let i_spec_unit = 2.0 * n_slope * vt * vt;
                    let i_norm = (i_f - i_r) * i_spec_unit;
                    let cd = beta * i_norm * clm;
                    let gm_val = beta * i_spec_unit * ((d_if_d_vp - d_ir_d_vp) / n_slope) * clm;
                    let gds_val = beta * i_spec_unit * d_ir_d_vp * clm + beta * i_norm * lambda;
                    let gmbs_val = -gm_val * d_von_d_vbs;

                    let d_w = kp * inv_l * i_norm * clm;
                    let d_l = -cd * inv_l;
                    let d_vto = -pol * gm_val;
                    let d_kp = w_over_l * i_norm * clm;
                    (cd, gm_val, gds_val, gmbs_val, d_w, d_l, d_vto, d_kp)
                } else {
                    // Standard SPICE3f5 Level-1 Shichman-Hodges model
                    let v_ov = vgs - von;
                    if v_ov <= 0.0 {
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
                    } else if vds < v_ov {
                        // Linear (triode) region
                        let core = vds * (v_ov - 0.5 * vds);
                        let cd = beta * core * clm;
                        let gm_val = beta * vds * clm;
                        let gds_val = beta * (v_ov - vds) * clm + beta * lambda * core;
                        let gmbs_val = -gm_val * d_von_d_vbs;
                        let d_w = kp * inv_l * core * clm;
                        let d_l = -cd * inv_l;
                        let d_vto = -pol * gm_val;
                        let d_kp = w_over_l * core * clm;
                        (cd, gm_val, gds_val, gmbs_val, d_w, d_l, d_vto, d_kp)
                    } else {
                        // Saturation region
                        let core = 0.5 * v_ov * v_ov;
                        let cd = beta * core * clm;
                        let gm_val = beta * v_ov * clm;
                        let gds_val = beta * lambda * core;
                        let gmbs_val = -gm_val * d_von_d_vbs;
                        let d_w = kp * inv_l * core * clm;
                        let d_l = -cd * inv_l;
                        let d_vto = -pol * gm_val;
                        let d_kp = w_over_l * core * clm;
                        (cd, gm_val, gds_val, gmbs_val, d_w, d_l, d_vto, d_kp)
                    }
                };

                // Signed drain-to-source current in global node orientation (with minimum Gmin bleed conductance)
                let gds = gds + gmin;
                let id_oriented = pol * mode * cdrain + gmin * (vd - vs);
                state.mos_id[i] = id_oriented;
                state.mos_gm[i] = gm;
                state.mos_gds[i] = gds;
                state.mos_gmbs[i] = gmbs;
                state.mos_did_dw[i] = pol * mode * did_dw;
                state.mos_did_dl[i] = pol * mode * did_dl;
                state.mos_did_dvto[i] = pol * mode * did_dvto;
                state.mos_did_dkp[i] = pol * mode * did_dkp;

                if mode > 0.0 {
                    // Normal mode: controlled by (V_G - V_S) and (V_B - V_S)
                    let ieq = id_oriented - gm * (vg - vs) - gmbs * (vb - vs) - gds * (vd - vs);
                    state.mos_ieq[i] = ieq;
                    state.cond_buf[state.mos_gm_buf_idx[i] as usize] = gm;
                    state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds;
                    state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = gmbs;
                    state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = 0.0;
                    state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = 0.0;
                    state.rhs_buf[state.mos_rhs_buf_idx[i] as usize] = ieq;
                } else {
                    // Reversed mode: controlled by (V_G - V_D) and (V_B - V_D)
                    let ieq = id_oriented + gm * (vg - vd) + gmbs * (vb - vd) - gds * (vd - vs);
                    state.mos_ieq[i] = ieq;
                    state.cond_buf[state.mos_gm_buf_idx[i] as usize] = 0.0;
                    state.cond_buf[state.mos_gds_buf_idx[i] as usize] = gds;
                    state.cond_buf[state.mos_gmbs_buf_idx[i] as usize] = 0.0;
                    state.cond_buf[state.mos_rev_gm_buf_idx[i] as usize] = -gm;
                    state.cond_buf[state.mos_rev_gmbs_buf_idx[i] as usize] = -gmbs;
                    state.rhs_buf[state.mos_rhs_buf_idx[i] as usize] = ieq;
                }
            }
        });
    }
}

impl ComputeBackend for CpuSimdBackend {
    fn backend_name(&self) -> &'static str {
        "cpu-pulp-simd"
    }

    fn evaluate_devices(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> SimResult<()> {
        // Update independent voltage & current source RHS slots
        for (i, &val) in state.vsrc_values.iter().enumerate() {
            state.rhs_buf[state.vsrc_rhs_buf_idx[i] as usize] = val;
        }
        for (i, &val) in state.isrc_values.iter().enumerate() {
            state.rhs_buf[state.isrc_rhs_buf_idx[i] as usize] = val;
        }

        // Update transient companion elements (Capacitors and Inductors)
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

        // Evaluate nonlinear semiconductor devices via SIMD
        let diodes_limited = self.eval_diodes(state, voltages);
        self.eval_mosfets(state, voltages);
        state.limiter_active = diodes_limited;

        Ok(())
    }

    fn stamp_jacobian(
        &self,
        state: &DeviceSoaBuffer,
        jacobian: &mut CscMatrix<f64>,
        rhs: &mut [f64],
    ) -> SimResult<()> {
        if !state.linear_dirty && state.static_linear_csc_values.len() == jacobian.values.len() {
            // Fast path: memcpy static linear conductances + gather only dynamic/nonlinear entries!
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
            // Full gather across all devices + diagonal Gmin
            if jacobian.values.len() >= 16_384 {
                let active_nnz = &state.gather_table.active_nnz;
                let nnz_terms = &state.gather_table.nnz_terms;
                let cond_buf = state.cond_buf.as_slice();
                jacobian.values.fill(0.0);
                let vals = &mut jacobian.values;
                for &(k, start, end) in active_nnz {
                    let mut acc = 0.0_f64;
                    for term in &nnz_terms[start as usize..end as usize] {
                        acc = term.sign.mul_add(cond_buf[term.buffer_idx as usize], acc);
                    }
                    vals[k as usize] = acc;
                }
                let _ = rayon::current_num_threads();
            } else {
                state.gather_table.gather_into(
                    &state.cond_buf,
                    &state.rhs_buf,
                    &mut jacobian.values,
                    rhs,
                );
            }
            for &diag in &state.dc_floating_diag_idx {
                if diag < jacobian.values.len() {
                    jacobian.values[diag] += 1e-12;
                }
            }
            let extra_gmin = state.gmin - 1e-12;
            if extra_gmin.abs() > 0.0 {
                jacobian.add_diagonal_gmin(state.num_nodes, extra_gmin);
            }

            if jacobian.values.len() >= 16_384 {
                rhs.fill(0.0);
                for &(i, start, end) in &state.gather_table.active_rhs {
                    let mut acc = 0.0_f64;
                    for term in &state.gather_table.rhs_terms[start as usize..end as usize] {
                        acc = term
                            .sign
                            .mul_add(state.rhs_buf[term.buffer_idx as usize], acc);
                    }
                    rhs[i as usize] = acc;
                }
            }
        }
        Ok(())
    }
}
