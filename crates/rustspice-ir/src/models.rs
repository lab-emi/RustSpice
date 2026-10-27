//! Canonical Semiconductor Device Physics Models in SSA IR (`Diode`, `MOSFET Level-1`, `EKV MOSFET`).
//!
//! Each builder defines only the forward physics equations ($I(V, \theta)$ and $Q(V, \theta)$),
//! then invokes automatic differentiation (`differentiate_forward` & `differentiate_reverse`)
//! and [`optimize_graph`] (CSE + DCE) to synthesize all Jacobian conductances, capacitances,
//! and parameter sensitivities automatically.

use crate::autodiff::DiffTarget;
use crate::opt::optimize_graph;
use crate::ssa::IrGraph;

/// Builds and optimizes the symbolic SSA IR for an intrinsic SPICE PN Junction Diode
/// including Shockley DC current, junction capacitance charge, Jacobian conductances,
/// and parameter sensitivities $\partial I_D / \partial I_S, \partial I_D / \partial N$.
pub fn build_diode_ir() -> IrGraph {
    let mut g = IrGraph::new("diode");

    // Input terminal voltage difference: v_d = v(anode) - v(cathode)
    let v_d = g.input("v_d");

    // Model parameters
    let is = g.param("is", 1e-14);
    let n = g.param("n", 1.0);
    let vt = g.param("vt", 0.025851997);
    let gmin = g.param("gmin", 1e-12);
    let cjo = g.param("cjo", 0.0);
    let tt = g.param("tt", 0.0);

    // Forward Shockley equation: i_d = is * (exp(v_d / (n * vt)) - 1.0) + gmin * v_d
    let one = g.const_val(1.0);
    let n_vt = g.mul(n, vt);
    let arg = g.div(v_d, n_vt);
    let exp_arg = g.exp(arg);
    let exp_m1 = g.sub(exp_arg, one);
    let i_shockley = g.mul(is, exp_m1);
    let i_leak = g.mul(gmin, v_d);
    let i_d = g.add(i_shockley, i_leak);

    // Charge equation: q_d = tt * i_shockley + cjo * v_d
    let q_diff = g.mul(tt, i_shockley);
    let q_depl = g.mul(cjo, v_d);
    let q_d = g.add(q_diff, q_depl);

    g.add_output("i_d", i_d);
    g.add_output("q_d", q_d);

    // Automatic differentiation for Jacobian & capacitance
    let g_d = g.differentiate_forward(i_d, &DiffTarget::Input("v_d".to_string()));
    let c_d = g.differentiate_forward(q_d, &DiffTarget::Input("v_d".to_string()));

    // Automatic differentiation for AI4EDA parameter sensitivities
    let did_dis = g.differentiate_forward(i_d, &DiffTarget::Param("is".to_string()));
    let did_dn = g.differentiate_forward(i_d, &DiffTarget::Param("n".to_string()));

    g.add_output("g_d", g_d);
    g.add_output("c_d", c_d);
    g.add_output("d_id_d_is", did_dis);
    g.add_output("d_id_d_n", did_dn);

    optimize_graph(&mut g);
    g
}

/// Builds and optimizes the branchless Shichman-Hodges Level-1 MOSFET SSA IR
/// using `Select` operations for cutoff, linear (triode), and saturation regions.
pub fn build_mosfet_level1_ir() -> IrGraph {
    let mut g = IrGraph::new("mosfet_level1");

    // Inputs: v_gs, v_ds
    let v_gs = g.input("v_gs");
    let v_ds = g.input("v_ds");

    // Parameters: kp, w, l, vto, lambda, gmin
    let kp = g.param("kp", 120e-6);
    let w = g.param("w", 10e-6);
    let l = g.param("l", 1e-6);
    let vto = g.param("vto", 0.7);
    let lambda = g.param("lambda", 0.02);
    let gmin = g.param("gmin", 1e-12);

    let zero = g.const_val(0.0);
    let one = g.const_val(1.0);
    let half = g.const_val(0.5);

    // beta = kp * (w / l)
    let w_over_l = g.div(w, l);
    let beta = g.mul(kp, w_over_l);

    // v_ov = v_gs - vto
    let v_ov = g.sub(v_gs, vto);

    // Channel length modulation factor: clm = 1.0 + lambda * v_ds
    let lam_vds = g.mul(lambda, v_ds);
    let clm = g.add(one, lam_vds);

    // Linear region current: beta * (v_ov * v_ds - 0.5 * v_ds^2) * clm
    let vov_vds = g.mul(v_ov, v_ds);
    let vds_sq = g.mul(v_ds, v_ds);
    let half_vds_sq = g.mul(half, vds_sq);
    let lin_core = g.sub(vov_vds, half_vds_sq);
    let lin_beta = g.mul(beta, lin_core);
    let i_lin = g.mul(lin_beta, clm);

    // Saturation region current: 0.5 * beta * v_ov^2 * clm
    let vov_sq = g.mul(v_ov, v_ov);
    let half_beta = g.mul(half, beta);
    let sat_beta = g.mul(half_beta, vov_sq);
    let i_sat = g.mul(sat_beta, clm);

    // Branchless region selection:
    // if v_ov > 0: (if v_ds > v_ov: i_sat else: i_lin) else: 0
    let is_on = g.gt(v_ov, zero);
    let is_sat = g.gt(v_ds, v_ov);
    let i_active = g.select(is_sat, i_sat, i_lin);
    let i_mos = g.select(is_on, i_active, zero);
    let i_leak = g.mul(gmin, v_ds);
    let i_d = g.add(i_mos, i_leak);

    g.add_output("i_d", i_d);

    // Auto-Diff Jacobians: g_m = d(i_d)/d(v_gs), g_ds = d(i_d)/d(v_ds)
    let g_m = g.differentiate_forward(i_d, &DiffTarget::Input("v_gs".to_string()));
    let g_ds = g.differentiate_forward(i_d, &DiffTarget::Input("v_ds".to_string()));

    // Auto-Diff Parameter Sensitivities: d(i_d)/d(w), d(i_d)/d(l), d(i_d)/d(vto)
    let did_dw = g.differentiate_forward(i_d, &DiffTarget::Param("w".to_string()));
    let did_dl = g.differentiate_forward(i_d, &DiffTarget::Param("l".to_string()));
    let did_dvto = g.differentiate_forward(i_d, &DiffTarget::Param("vto".to_string()));

    g.add_output("g_m", g_m);
    g.add_output("g_ds", g_ds);
    g.add_output("d_id_d_w", did_dw);
    g.add_output("d_id_d_l", did_dl);
    g.add_output("d_id_d_vto", did_dvto);

    optimize_graph(&mut g);
    g
}

/// Builds and optimizes the continuous all-region Enz-Krummenacher-Vittoz (EKV) MOSFET IR,
/// providing $C^\infty$ smooth transitions across subthreshold, weak, moderate, and strong inversion.
pub fn build_mosfet_ekv_ir() -> IrGraph {
    let mut g = IrGraph::new("mosfet_ekv");

    let v_gs = g.input("v_gs");
    let v_ds = g.input("v_ds");

    let kp = g.param("kp", 120e-6);
    let w = g.param("w", 10e-6);
    let l = g.param("l", 1e-6);
    let vto = g.param("vto", 0.7);
    let n_slope = g.param("n", 1.3);
    let vt = g.param("vt", 0.025851997);
    let lambda = g.param("lambda", 0.02);

    let one = g.const_val(1.0);
    let two = g.const_val(2.0);

    // Specific current I_spec = 2 * n * kp * (W/L) * Vt^2
    let w_over_l = g.div(w, l);
    let beta = g.mul(kp, w_over_l);
    let two_n = g.mul(two, n_slope);
    let two_n_beta = g.mul(two_n, beta);
    let vt_sq = g.mul(vt, vt);
    let i_spec = g.mul(two_n_beta, vt_sq);

    // Pinch-off voltage V_P = (V_GS - V_TO) / n
    let v_ov = g.sub(v_gs, vto);
    let v_p = g.div(v_ov, n_slope);

    // Forward normalized current: i_f = (ln(1 + exp(V_P / (2 * Vt))))^2
    let two_vt = g.mul(two, vt);
    let arg_f = g.div(v_p, two_vt);
    let exp_f = g.exp(arg_f);
    let one_p_exp_f = g.add(one, exp_f);
    let ln_f = g.ln(one_p_exp_f);
    let i_f = g.mul(ln_f, ln_f);

    // Reverse normalized current: i_r = (ln(1 + exp((V_P - V_DS) / (2 * Vt))))^2
    let vp_m_vds = g.sub(v_p, v_ds);
    let arg_r = g.div(vp_m_vds, two_vt);
    let exp_r = g.exp(arg_r);
    let one_p_exp_r = g.add(one, exp_r);
    let ln_r = g.ln(one_p_exp_r);
    let i_r = g.mul(ln_r, ln_r);

    // Net drain current: I_D = I_spec * (i_f - i_r) * (1 + lambda * V_DS)
    let diff_ir = g.sub(i_f, i_r);
    let i_base = g.mul(i_spec, diff_ir);
    let lam_vds = g.mul(lambda, v_ds);
    let clm = g.add(one, lam_vds);
    let i_d = g.mul(i_base, clm);

    g.add_output("i_d", i_d);

    let g_m = g.differentiate_forward(i_d, &DiffTarget::Input("v_gs".to_string()));
    let g_ds = g.differentiate_forward(i_d, &DiffTarget::Input("v_ds".to_string()));
    let did_dw = g.differentiate_forward(i_d, &DiffTarget::Param("w".to_string()));
    let did_dl = g.differentiate_forward(i_d, &DiffTarget::Param("l".to_string()));
    let did_dvto = g.differentiate_forward(i_d, &DiffTarget::Param("vto".to_string()));

    g.add_output("g_m", g_m);
    g.add_output("g_ds", g_ds);
    g.add_output("d_id_d_w", did_dw);
    g.add_output("d_id_d_l", did_dl);
    g.add_output("d_id_d_vto", did_dvto);

    optimize_graph(&mut g);
    g
}
