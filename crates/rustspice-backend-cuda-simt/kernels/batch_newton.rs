// Batched DC Newton solve of one circuit topology at many parameter corners.
//
// This file is compiled twice from the same source: `include!`d by `rustspice-backend-cuda-simt` (the `cpu_ref`
// executor, one corner per rayon task) and by the cuda-oxide kernel crate in `kernels/gpu` (one corner per CUDA
// thread). It uses `core` and the `f64` math methods only (libm on the CPU, libdevice on the GPU) and never
// allocates.
//
// The plan is the CPU solver's own compiled data: the Jacobian / RHS gather tables of `DeviceSoaBuffer`, the
// fill-reducing order and static-pivot elimination stream of `UnrolledSymbolicLu`, and the base `cond_buf` /
// `rhs_buf` of every linear device. The Newton loop is `NonlinearSolver::solve_in_place` without its Shamanskii
// shortcut: the same junction limiting, KCL residual test, step test and source voltage box, so an accepted corner
// meets the CPU solver's convergence criteria. The devices are evaluated in the CPU backend's order (plain diodes,
// level-1 MOSFETs, then the plain junctions, which add onto the drain-source slots of the MOSFET they are folded
// into).
//
// The kernel is bound by memory traffic, so a corner keeps only what differs between corners: device values that
// are the same everywhere (resistors and sources that no parameter changes) are summed into the plan once
// (`F_JAC_STATIC`, `F_RHS_STATIC`), and a corner stores only the "dynamic" values of its diodes and parameters. The
// Jacobian is gathered straight into the LU array and the residual is formed row by row in registers.
//
// Per-corner state lives in a workspace of `f64` slots, slot `s` of corner `c` at `w[s * stride + c]`: the GPU uses
// `stride = corners` (neighbouring threads touch neighbouring words), the CPU one corner at a time with `stride = 1`.

/// Offsets and sizes in the `u32` plan header.
pub mod hdr {
    /// MNA dimension.
    pub const N: usize = 0;
    pub const NUM_NODES: usize = 1;
    /// Jacobian non-zeros (CSC values).
    pub const NNZ: usize = 2;
    pub const LU_NNZ: usize = 3;
    /// Per-corner conductance / right-hand-side values (diodes and parameters).
    pub const NDCOND: usize = 4;
    pub const NDRHS: usize = 5;
    pub const NDIODE: usize = 6;
    pub const NOPS: usize = 7;
    pub const NPARAM: usize = 8;
    pub const MAX_ITERS: usize = 9;
    pub const NVSRC: usize = 10;
    /// LU entries that are fill-in (zeroed before each factorization; the others are overwritten by the gather).
    pub const NFILL: usize = 11;
    /// 1: the voltage box is the source range itself (no current sources, no VCCS).
    pub const BOX_TIGHT: usize = 12;
    pub const BOX_RELEASE: usize = 13;
    // offsets of the `u32` arrays
    /// Rows of the Jacobian (`n + 1` pointers into `ROW_COL` / `ROW_LU`, columns ascending in each row).
    pub const ROW_PTR: usize = 14;
    pub const ROW_COL: usize = 15;
    /// LU position of each row entry (where the gather stored it).
    pub const ROW_LU: usize = 16;
    /// Dynamic gather terms of each CSC non-zero (`nnz + 1` pointers into `JAC_DSRC`).
    pub const JAC_DPTR: usize = 17;
    /// Dynamic conductance slot of each term.
    pub const JAC_DSRC: usize = 18;
    /// LU position of each CSC non-zero.
    pub const JAC_LU: usize = 19;
    /// Dynamic gather terms of each RHS row (`n + 1` pointers into `RHS_DSRC`).
    pub const RHS_DPTR: usize = 20;
    pub const RHS_DSRC: usize = 21;
    /// 4 words per operation: kind, a, b, c.
    pub const OPS: usize = 22;
    pub const DIAG: usize = 23;
    pub const L_PTR: usize = 24;
    /// 2 words per term: vector index, LU index.
    pub const L_TERMS: usize = 25;
    pub const U_PTR: usize = 26;
    pub const U_TERMS: usize = 27;
    pub const FILL: usize = 28;
    /// Position in the factors' right-hand side of each residual row (static row pivots composed with the
    /// fill-reducing order).
    pub const ROW_POS: usize = 29;
    /// Position of each unknown in the solution vector of the factors (the fill-reducing column order).
    pub const X_PERM: usize = 30;
    /// 4 words per diode: anode node, cathode node (0 = ground), dynamic conductance slot, dynamic RHS slot.
    pub const DIODES: usize = 31;
    /// 2 words per parameter slot: kind, index (a dynamic slot for `COND` / `RHSB`, a diode for the others).
    pub const PARAMS: usize = 32;
    /// Per voltage source: 1 + its dynamic RHS slot, or 0 when its value is static (`F_VSRC`).
    pub const VSRC_DYN: usize = 33;
    // offsets of the `f64` arrays
    /// Sum of the static gather terms of each CSC non-zero (floating-node conductance included).
    pub const F_JAC_STATIC: usize = 34;
    pub const F_JAC_DSIGN: usize = 35;
    pub const F_RHS_STATIC: usize = 36;
    pub const F_RHS_DSIGN: usize = 37;
    /// Base values of the dynamic slots.
    pub const F_DCOND0: usize = 38;
    pub const F_DRHS0: usize = 39;
    pub const F_DIODE_IS: usize = 40;
    pub const F_DIODE_N: usize = 41;
    /// Static voltage source values.
    pub const F_VSRC: usize = 42;
    /// Level-1 MOSFETs and plain junctions (MOSFET bulk junctions).
    pub const NMOS: usize = 43;
    pub const NJUNC: usize = 44;
    /// 10 words per MOSFET: drain, gate, source, bulk, the dynamic conductance slots of `gm`, `gds`, `gmbs`,
    /// reversed `gm`, reversed `gmbs`, and the dynamic RHS slot of its current.
    pub const MOS: usize = 45;
    /// 5 words per MOSFET (`W`, `L`, `VTO`, `KP`, `LAMBDA`): 0 for the plan's value (`F_MOS`), else 1 + the
    /// parameter slot that holds the corner's value.
    pub const MOS_PARAM: usize = 46;
    /// 5 words per junction: anode, cathode, dynamic conductance slot, dynamic RHS slot, 1 when it is folded into
    /// a MOSFET's drain-source slots (adds onto them) or 0 (owns them).
    pub const JUNCS: usize = 47;
    /// 8 values per MOSFET: polarity, `KP`, `W`, `L` (effective), `VTO`, `GAMMA`, `PHI`, `LAMBDA`.
    pub const F_MOS: usize = 48;
    /// 6 values per junction: `is`, `n Vt`, `1 / (n Vt)`, `is / (n Vt)`, critical voltage, fold sign.
    pub const F_JUNC: usize = 49;
    pub const LEN: usize = 50;
}

/// Scalars in the `f64` plan header.
pub mod fhdr {
    pub const VT: usize = 0;
    pub const GMIN: usize = 1;
    pub const RELTOL: usize = 2;
    pub const VNTOL: usize = 3;
    pub const ABSTOL: usize = 4;
    pub const RES_RELTOL: usize = 5;
    pub const LEN: usize = 6;
}

/// Kinds of the elimination operations (`UnrolledSymbolicLu::factor_ops`).
pub mod op {
    pub const INVERT_PIVOT: u32 = 0;
    pub const SCALE_MULTIPLIER: u32 = 1;
    pub const ELIMINATE: u32 = 2;
}

/// Kinds of the per-corner parameter slots.
pub mod param {
    /// Dynamic conductance slot `index` (a resistor's conductance).
    pub const COND: u32 = 0;
    /// Dynamic RHS slot `index` (an independent source's DC value).
    pub const RHSB: u32 = 1;
    /// Saturation current of diode `index`.
    pub const DIODE_IS: u32 = 2;
    /// Emission coefficient of diode `index`.
    pub const DIODE_N: u32 = 3;
    /// `W`, `L`, `VTO`, `KP`, `LAMBDA` of MOSFET `index` (read from the parameter array where the MOSFET is
    /// evaluated, not copied into the workspace).
    pub const MOS_W: u32 = 4;
    pub const MOS_L: u32 = 5;
    pub const MOS_VTO: u32 = 6;
    pub const MOS_KP: u32 = 7;
    pub const MOS_LAMBDA: u32 = 8;
}

/// Outcome of one corner.
pub mod status {
    pub const CONVERGED: u32 = 0;
    pub const NO_CONVERGENCE: u32 = 1;
    /// Zero, non-finite or unstable pivot of the static-pivot factorization.
    pub const SINGULAR: u32 = 2;
    pub const NON_FINITE: u32 = 3;
    /// A junction needs the linear continuation of the exponential: the CPU solver reports an error there.
    pub const EXP_RANGE: u32 = 4;
}

/// `exp` argument above which the diode exponential is continued linearly (`DIODE_EXP_ARG_MAX`).
pub const DIODE_EXP_ARG_MAX: f64 = 600.0;
const DIODE_EXP_SLOPE_MAX: f64 = 3.773_020_300_929_939_7e260;
/// Growth bound of the static-pivot factors (`UnrolledSymbolicLu::MAX_FACTOR_GROWTH`).
const MAX_FACTOR_GROWTH: f64 = 1e12;

/// Number of `f64` workspace slots of one corner.
pub fn workspace_slots(pu: &[u32]) -> usize {
    2 * pu[hdr::N] as usize
        + pu[hdr::NDCOND] as usize
        + pu[hdr::NDRHS] as usize
        + pu[hdr::LU_NNZ] as usize
        + 3 * pu[hdr::NDIODE] as usize
        + pu[hdr::NJUNC] as usize
}

/// Slot offsets of the workspace.
#[derive(Clone, Copy)]
pub struct Slots {
    pub x: usize,
    pub dx: usize,
    pub dcond: usize,
    pub drhs: usize,
    pub lu: usize,
    pub vd: usize,
    pub dis: usize,
    pub dn: usize,
    /// Junction voltages of the last evaluation (the step limiter's reference).
    pub vj: usize,
}

impl Slots {
    pub fn new(pu: &[u32]) -> Slots {
        let n = pu[hdr::N] as usize;
        let x = 0;
        let dx = x + n;
        let dcond = dx + n;
        let drhs = dcond + pu[hdr::NDCOND] as usize;
        let lu = drhs + pu[hdr::NDRHS] as usize;
        let vd = lu + pu[hdr::LU_NNZ] as usize;
        let dis = vd + pu[hdr::NDIODE] as usize;
        let dn = dis + pu[hdr::NDIODE] as usize;
        let vj = dn + pu[hdr::NDIODE] as usize;
        Slots {
            x,
            dx,
            dcond,
            drhs,
            lu,
            vd,
            dis,
            dn,
            vj,
        }
    }
}

/// One corner's view of the workspace.
#[derive(Clone, Copy)]
pub struct Work {
    pub w: *mut f64,
    pub stride: usize,
    pub corner: usize,
}

impl Work {
    #[inline(always)]
    unsafe fn ld(self, s: usize) -> f64 {
        unsafe { *self.w.add(s * self.stride + self.corner) }
    }
    #[inline(always)]
    unsafe fn st(self, s: usize, v: f64) {
        unsafe { *self.w.add(s * self.stride + self.corner) = v }
    }
    /// Voltage of `node` (0 = ground) in the vector at slot `base`.
    #[inline(always)]
    unsafe fn node(self, base: usize, node: u32) -> f64 {
        if node == 0 {
            0.0
        } else {
            unsafe { self.ld(base + node as usize - 1) }
        }
    }
}

/// SPICE junction step limiter (`pnjlim`).
#[inline(always)]
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

/// `exp(arg)` and its derivative, continued linearly above [`DIODE_EXP_ARG_MAX`].
#[inline(always)]
pub fn diode_exp(arg: f64) -> (f64, f64) {
    if arg <= DIODE_EXP_ARG_MAX {
        let e = arg.exp();
        (e, e)
    } else {
        (
            DIODE_EXP_SLOPE_MAX * (1.0 + (arg - DIODE_EXP_ARG_MAX)),
            DIODE_EXP_SLOPE_MAX,
        )
    }
}

/// Junction current and conductance of a plain diode at `vd`: the CPU backend's `eval_diodes`, operation for
/// operation (fused multiply-adds included), so both give the same bits.
#[inline(always)]
pub fn diode_current(vd: f64, is: f64, n_vt: f64, gmin: f64) -> (f64, f64) {
    let inv_n_vt = 1.0 / n_vt;
    if vd >= -3.0 * n_vt {
        let (ev, d_ev) = diode_exp(vd * inv_n_vt);
        (
            is.mul_add(ev - 1.0, gmin * vd),
            (is * inv_n_vt).mul_add(d_ev, gmin),
        )
    } else {
        let arg = 3.0 * n_vt / (vd * core::f64::consts::E);
        let arg3 = arg * arg * arg;
        (
            (-is).mul_add(1.0 + arg3, gmin * vd),
            (is * 3.0 * arg3 / vd) + gmin,
        )
    }
}

/// Level-1 (Shichman-Hodges) MOSFET at the node voltages `vd`, `vg`, `vs`, `vb`: the CPU backend's
/// `eval_mosfets`, operation for operation. Returns `gm`, `gds`, `gmbs`, the Norton current and whether the device
/// runs reversed (drain and source swapped).
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn mos_level1(
    vd: f64,
    vg: f64,
    vs: f64,
    vb: f64,
    pol: f64,
    kp: f64,
    w: f64,
    l: f64,
    vto: f64,
    gamma: f64,
    phi: f64,
    lambda: f64,
) -> (f64, f64, f64, f64, bool) {
    let vds_signed = pol * (vd - vs);
    let (mode, vds, vgs, vbs) = if vds_signed >= 0.0 {
        (1.0_f64, vds_signed, pol * (vg - vs), pol * (vb - vs))
    } else {
        (-1.0_f64, -vds_signed, pol * (vg - vd), pol * (vb - vd))
    };
    let inv_l = 1.0 / l;
    let beta = kp * (w * inv_l);
    let phi = phi.max(0.0);
    let vt0 = pol * vto;
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
    let v_ov = vgs - von;
    let (cdrain, gm, gds) = if v_ov <= 0.0 {
        (0.0, 0.0, 0.0)
    } else if vds < v_ov {
        let core = vds * (v_ov - 0.5 * vds);
        (
            beta * core * clm,
            beta * vds * clm,
            beta * (v_ov - vds) * clm + beta * lambda * core,
        )
    } else {
        let core = 0.5 * v_ov * v_ov;
        (beta * core * clm, beta * v_ov * clm, beta * lambda * core)
    };
    let gmbs = -gm * d_von_d_vbs;
    let id = pol * mode * cdrain;
    if mode > 0.0 {
        let ieq = id - gm * (vg - vs) - gmbs * (vb - vs) - gds * (vd - vs);
        (gm, gds, gmbs, ieq, false)
    } else {
        let ieq = id + gm * (vg - vd) + gmbs * (vb - vd) - gds * (vd - vs);
        (gm, gds, gmbs, ieq, true)
    }
}

/// Conductance and Norton current of a plain junction at `vd` (`BulkJunctions::evaluate` of the CPU backend):
/// `is (exp(vd / nvt) - 1) + gmin vd`, with `exp - 1` from its Taylor series near zero bias.
#[inline(always)]
pub fn junction_current(
    vd: f64,
    is: f64,
    nvt: f64,
    inv_nvt: f64,
    g_sat: f64,
    gmin: f64,
) -> (f64, f64) {
    if vd < -40.0 * nvt {
        (gmin, -is)
    } else {
        let arg = vd * inv_nvt;
        let (em1, e) = if arg.abs() < 1e-3 {
            let em1 = arg
                * (1.0
                    + arg * (0.5 + arg * (1.0 / 6.0 + arg * (1.0 / 24.0 + arg * (1.0 / 120.0)))));
            (em1, 1.0 + em1)
        } else {
            let (e, de) = diode_exp(arg);
            (e - 1.0, de)
        };
        let g = g_sat * e + gmin;
        (g, is * em1 + gmin * vd - g * vd)
    }
}

/// Solves one corner; the solution is left in the `x` slots. Returns the [`status`] and the Newton iterations.
///
/// # Safety
/// `work.w` must hold [`workspace_slots`] slots of `work.stride` words each, `params` the parameter slots of every
/// corner (slot `j` of this corner at `params[j * pstride + pcorner]`), and `pu` / `pf` a plan built for this
/// workspace.
pub unsafe fn solve_corner(
    pu: &[u32],
    pf: &[f64],
    params: &[f64],
    pstride: usize,
    pcorner: usize,
    work: Work,
) -> (u32, u32) {
    unsafe { solve_corner_inner(pu, pf, params, pstride, pcorner, work) }
}

#[inline(always)]
unsafe fn solve_corner_inner(
    pu: &[u32],
    pf: &[f64],
    params: &[f64],
    pstride: usize,
    pcorner: usize,
    work: Work,
) -> (u32, u32) {
    let s = Slots::new(pu);
    let n = pu[hdr::N] as usize;
    let num_nodes = pu[hdr::NUM_NODES] as usize;
    let nnz = pu[hdr::NNZ] as usize;
    let ndcond = pu[hdr::NDCOND] as usize;
    let ndrhs = pu[hdr::NDRHS] as usize;
    let ndiode = pu[hdr::NDIODE] as usize;
    let nops = pu[hdr::NOPS] as usize;
    let nparam = pu[hdr::NPARAM] as usize;
    let max_iters = pu[hdr::MAX_ITERS];
    let nvsrc = pu[hdr::NVSRC] as usize;
    let nfill = pu[hdr::NFILL] as usize;
    let box_tight = pu[hdr::BOX_TIGHT] != 0;
    let box_release = pu[hdr::BOX_RELEASE];
    let row_ptr = pu[hdr::ROW_PTR] as usize;
    let row_col = pu[hdr::ROW_COL] as usize;
    let row_lu = pu[hdr::ROW_LU] as usize;
    let jac_dptr = pu[hdr::JAC_DPTR] as usize;
    let jac_dsrc = pu[hdr::JAC_DSRC] as usize;
    let jac_lu = pu[hdr::JAC_LU] as usize;
    let rhs_dptr = pu[hdr::RHS_DPTR] as usize;
    let rhs_dsrc = pu[hdr::RHS_DSRC] as usize;
    let ops = pu[hdr::OPS] as usize;
    let diag = pu[hdr::DIAG] as usize;
    let l_ptr = pu[hdr::L_PTR] as usize;
    let l_terms = pu[hdr::L_TERMS] as usize;
    let u_ptr = pu[hdr::U_PTR] as usize;
    let u_terms = pu[hdr::U_TERMS] as usize;
    let fill = pu[hdr::FILL] as usize;
    let row_pos = pu[hdr::ROW_POS] as usize;
    let x_perm = pu[hdr::X_PERM] as usize;
    let diodes = pu[hdr::DIODES] as usize;
    let pslots = pu[hdr::PARAMS] as usize;
    let vsrc_dyn = pu[hdr::VSRC_DYN] as usize;
    let f_jac_static = pu[hdr::F_JAC_STATIC] as usize;
    let f_jac_dsign = pu[hdr::F_JAC_DSIGN] as usize;
    let f_rhs_static = pu[hdr::F_RHS_STATIC] as usize;
    let f_rhs_dsign = pu[hdr::F_RHS_DSIGN] as usize;
    let f_dcond0 = pu[hdr::F_DCOND0] as usize;
    let f_drhs0 = pu[hdr::F_DRHS0] as usize;
    let f_dis = pu[hdr::F_DIODE_IS] as usize;
    let f_dn = pu[hdr::F_DIODE_N] as usize;
    let f_vsrc = pu[hdr::F_VSRC] as usize;
    let nmos = pu[hdr::NMOS] as usize;
    let njunc = pu[hdr::NJUNC] as usize;
    let mos = pu[hdr::MOS] as usize;
    let mos_param = pu[hdr::MOS_PARAM] as usize;
    let juncs = pu[hdr::JUNCS] as usize;
    let f_mos = pu[hdr::F_MOS] as usize;
    let f_junc = pu[hdr::F_JUNC] as usize;
    let vt = pf[fhdr::VT];
    let gmin = pf[fhdr::GMIN];
    let reltol = pf[fhdr::RELTOL];
    let vntol = pf[fhdr::VNTOL];
    let abstol = pf[fhdr::ABSTOL];
    let res_reltol = pf[fhdr::RES_RELTOL];

    unsafe {
        // base state of the dynamic values, then this corner's parameters
        for k in 0..ndcond {
            work.st(s.dcond + k, pf[f_dcond0 + k]);
        }
        for k in 0..ndrhs {
            work.st(s.drhs + k, pf[f_drhs0 + k]);
        }
        for d in 0..ndiode {
            work.st(s.dis + d, pf[f_dis + d]);
            work.st(s.dn + d, pf[f_dn + d]);
            work.st(s.vd + d, 0.0);
        }
        for j in 0..nparam {
            let v = params[j * pstride + pcorner];
            let kind = pu[pslots + 2 * j];
            let idx = pu[pslots + 2 * j + 1] as usize;
            if kind == param::COND {
                work.st(s.dcond + idx, v);
            } else if kind == param::RHSB {
                work.st(s.drhs + idx, v);
            } else if kind == param::DIODE_IS {
                work.st(s.dis + idx, v);
            } else if kind == param::DIODE_N {
                work.st(s.dn + idx, v);
            }
        }
        for j in 0..njunc {
            work.st(s.vj + j, 0.0);
        }
        for i in 0..n {
            work.st(s.x + i, 0.0);
        }

        // the source voltage box of `solve_in_place` (a DC operating point)
        let mut v_min = 0.0_f64;
        let mut v_max = 0.0_f64;
        for k in 0..nvsrc {
            let dynamic = pu[vsrc_dyn + k] as usize;
            let v = if dynamic > 0 {
                work.ld(s.drhs + dynamic - 1)
            } else {
                pf[f_vsrc + k]
            };
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
        let (box_lo, box_hi) = if box_tight && (v_max - v_min) > 0.0 {
            (v_min, v_max)
        } else {
            let span = (v_max - v_min).max(5.0);
            (v_min - span, v_max + span)
        };
        let mut box_active = true;
        let mut box_streak = 0u32;
        let mut prev_step_converged = false;

        let mut iter = 1u32;
        while iter <= max_iters {
            // 1. devices: plain diodes (`eval_diodes`, step limiting on)
            let mut limited = false;
            for d in 0..ndiode {
                let anode = pu[diodes + 4 * d];
                let cathode = pu[diodes + 4 * d + 1];
                let vd_raw = work.node(s.x, anode) - work.node(s.x, cathode);
                let is = work.ld(s.dis + d);
                let n_vt = work.ld(s.dn + d) * vt;
                let vcrit = n_vt * (n_vt / (core::f64::consts::SQRT_2 * is)).ln();
                let vd = pnjlim(vd_raw, work.ld(s.vd + d), n_vt, vcrit);
                if (vd - vd_raw).abs() > 1e-14 || vd / n_vt > DIODE_EXP_ARG_MAX {
                    limited = true;
                }
                work.st(s.vd + d, vd);
                let (id, gd) = diode_current(vd, is, n_vt, gmin);
                work.st(s.dcond + pu[diodes + 4 * d + 2] as usize, gd);
                work.st(s.drhs + pu[diodes + 4 * d + 3] as usize, id - gd * vd);
            }
            // level-1 MOSFETs (no step limiting, as on the CPU); a value the corner overrides comes from `params`
            for m in 0..nmos {
                let u = mos + 10 * m;
                let fm = f_mos + 8 * m;
                let mp = mos_param + 5 * m;
                let (gm, gds, gmbs, ieq, reversed) = mos_level1(
                    work.node(s.x, pu[u]),
                    work.node(s.x, pu[u + 1]),
                    work.node(s.x, pu[u + 2]),
                    work.node(s.x, pu[u + 3]),
                    pf[fm],
                    corner_value(pu[mp + 3], pf[fm + 1], params, pstride, pcorner),
                    corner_value(pu[mp], pf[fm + 2], params, pstride, pcorner),
                    corner_value(pu[mp + 1], pf[fm + 3], params, pstride, pcorner),
                    corner_value(pu[mp + 2], pf[fm + 4], params, pstride, pcorner),
                    pf[fm + 5],
                    pf[fm + 6],
                    corner_value(pu[mp + 4], pf[fm + 7], params, pstride, pcorner),
                );
                let (fwd_gm, fwd_gmbs, rev_gm, rev_gmbs) = if reversed {
                    (0.0, 0.0, -gm, -gmbs)
                } else {
                    (gm, gmbs, 0.0, 0.0)
                };
                work.st(s.dcond + pu[u + 4] as usize, fwd_gm);
                work.st(s.dcond + pu[u + 5] as usize, gds);
                work.st(s.dcond + pu[u + 6] as usize, fwd_gmbs);
                work.st(s.dcond + pu[u + 7] as usize, rev_gm);
                work.st(s.dcond + pu[u + 8] as usize, rev_gmbs);
                work.st(s.drhs + pu[u + 9] as usize, ieq);
            }
            // plain junctions (after the MOSFETs, whose drain-source slots the folded ones add onto)
            for j in 0..njunc {
                let u = juncs + 5 * j;
                let fj = f_junc + 6 * j;
                let raw = work.node(s.x, pu[u]) - work.node(s.x, pu[u + 1]);
                let (is, nvt, vcrit) = (pf[fj], pf[fj + 1], pf[fj + 4]);
                let vd = if raw > vcrit {
                    let v = pnjlim(raw, work.ld(s.vj + j), nvt, vcrit);
                    if v != raw || v > DIODE_EXP_ARG_MAX * nvt {
                        limited = true;
                    }
                    v
                } else {
                    raw
                };
                work.st(s.vj + j, vd);
                let (g, ieq) = junction_current(vd, is, nvt, pf[fj + 2], pf[fj + 3], gmin);
                let c = s.dcond + pu[u + 2] as usize;
                let r = s.drhs + pu[u + 3] as usize;
                if pu[u + 4] == 0 {
                    work.st(c, g);
                    work.st(r, ieq);
                } else {
                    work.st(c, work.ld(c) + g);
                    work.st(r, work.ld(r) + pf[fj + 5] * ieq);
                }
            }

            // 2. Jacobian straight into the LU array: static sum plus the dynamic terms; max |J| for the growth test
            for f in 0..nfill {
                work.st(s.lu + pu[fill + f] as usize, 0.0);
            }
            let mut max_a = 0.0_f64;
            let mut a_finite = true;
            for k in 0..nnz {
                let mut acc = pf[f_jac_static + k];
                for t in pu[jac_dptr + k] as usize..pu[jac_dptr + k + 1] as usize {
                    acc += pf[f_jac_dsign + t] * work.ld(s.dcond + pu[jac_dsrc + t] as usize);
                }
                work.st(s.lu + pu[jac_lu + k] as usize, acc);
                if !acc.is_finite() {
                    a_finite = false;
                }
                max_a = max_a.max(acc.abs());
            }

            // 3. KCL residual r = b - J x row by row (columns ascending, as the CPU's column sweep adds them), with
            // the magnitude of the summed terms; r goes to its position in the factors' right-hand side
            let mut res_ok = true;
            for i in 0..n {
                let mut b = pf[f_rhs_static + i];
                for t in pu[rhs_dptr + i] as usize..pu[rhs_dptr + i + 1] as usize {
                    b += pf[f_rhs_dsign + t] * work.ld(s.drhs + pu[rhs_dsrc + t] as usize);
                }
                let mut acc = 0.0_f64;
                let mut mag = 0.0_f64;
                for e in pu[row_ptr + i] as usize..pu[row_ptr + i + 1] as usize {
                    let term = work.ld(s.lu + pu[row_lu + e] as usize)
                        * work.ld(s.x + pu[row_col + e] as usize);
                    acc += term;
                    mag += term.abs();
                }
                let r = b - acc;
                let tol = abstol + res_reltol * (mag + b.abs());
                if r.abs() > tol || r.is_nan() {
                    res_ok = false;
                }
                work.st(s.dx + pu[row_pos + i] as usize, r);
            }
            if iter > 1 && prev_step_converged && res_ok && !limited {
                return (exp_range_status(pu, pf, work, s, vt), iter - 1);
            }

            // 4. static-pivot LU of J (`UnrolledSymbolicLu::factorize`)
            for o in 0..nops {
                let kind = pu[ops + 4 * o];
                let a = pu[ops + 4 * o + 1] as usize;
                let b = pu[ops + 4 * o + 2] as usize;
                if kind == op::INVERT_PIVOT {
                    let piv = work.ld(s.lu + a);
                    if piv.abs() < 1e-30 || !piv.is_finite() {
                        return (status::SINGULAR, iter);
                    }
                    work.st(s.lu + a, 1.0 / piv);
                } else if kind == op::SCALE_MULTIPLIER {
                    work.st(s.lu + a, work.ld(s.lu + a) * work.ld(s.lu + b));
                } else {
                    let c = pu[ops + 4 * o + 3] as usize;
                    let prod = work.ld(s.lu + b) * work.ld(s.lu + c);
                    work.st(s.lu + a, work.ld(s.lu + a) - prod);
                }
            }
            if !a_finite || !factors_are_stable(pu, work, s, max_a) {
                return (status::SINGULAR, iter);
            }

            // 5. dx = J^-1 r (residual form, as on the CPU), in the column order of the factors
            for k in 0..n {
                let xk = work.ld(s.dx + k);
                if xk != 0.0 {
                    for t in pu[l_ptr + k] as usize..pu[l_ptr + k + 1] as usize {
                        let vi = pu[l_terms + 2 * t] as usize;
                        let li = pu[l_terms + 2 * t + 1] as usize;
                        work.st(s.dx + vi, work.ld(s.dx + vi) - work.ld(s.lu + li) * xk);
                    }
                }
            }
            let mut k = n;
            while k > 0 {
                k -= 1;
                let xk = work.ld(s.dx + k) * work.ld(s.lu + pu[diag + k] as usize);
                work.st(s.dx + k, xk);
                if xk != 0.0 {
                    for t in pu[u_ptr + k] as usize..pu[u_ptr + k + 1] as usize {
                        let vi = pu[u_terms + 2 * t] as usize;
                        let ui = pu[u_terms + 2 * t + 1] as usize;
                        work.st(s.dx + vi, work.ld(s.dx + vi) - work.ld(s.lu + ui) * xk);
                    }
                }
            }

            // 6. step test and voltage box; x is updated in place (each unknown on its own)
            let mut step_ok = true;
            let mut box_binding = false;
            let mut max_step = 0.0_f64;
            for i in 0..n {
                let xi = work.ld(s.x + i);
                let next = xi + work.ld(s.dx + pu[x_perm + i] as usize);
                let raw_dx = next - xi;
                if !raw_dx.is_finite() {
                    return (status::NON_FINITE, iter);
                }
                let dx_abs = raw_dx.abs();
                max_step = max_step.max(dx_abs);
                let tol = reltol * next.abs().max(xi.abs()) + vntol;
                if dx_abs > tol {
                    step_ok = false;
                }
                let mut stored = next;
                if i < num_nodes && box_active {
                    let projected = next.max(box_lo).min(box_hi);
                    if (next - projected).abs() > tol {
                        box_binding = true;
                    }
                    stored = projected;
                }
                work.st(s.x + i, stored);
            }
            if box_binding {
                box_streak += 1;
                if box_streak >= box_release {
                    box_active = false;
                }
            } else {
                box_streak = 0;
            }
            prev_step_converged = step_ok && !box_binding;
            if step_ok && res_ok && max_step <= vntol && !limited && !box_binding {
                return (exp_range_status(pu, pf, work, s, vt), iter);
            }
            iter += 1;
        }
        (status::NO_CONVERGENCE, max_iters)
    }
}

/// The plan's value `plan`, or the corner's value of parameter slot `slot - 1` when `slot > 0`.
#[inline(always)]
fn corner_value(slot: u32, plan: f64, params: &[f64], pstride: usize, pcorner: usize) -> f64 {
    if slot == 0 {
        plan
    } else {
        params[(slot as usize - 1) * pstride + pcorner]
    }
}

/// [`status::EXP_RANGE`] when an accepted iterate puts a junction beyond the exponential's range.
#[inline(always)]
unsafe fn exp_range_status(pu: &[u32], pf: &[f64], work: Work, s: Slots, vt: f64) -> u32 {
    let diodes = pu[hdr::DIODES] as usize;
    for d in 0..pu[hdr::NDIODE] as usize {
        let vd =
            unsafe { work.node(s.x, pu[diodes + 4 * d]) - work.node(s.x, pu[diodes + 4 * d + 1]) };
        if vd / (unsafe { work.ld(s.dn + d) } * vt) > DIODE_EXP_ARG_MAX {
            return status::EXP_RANGE;
        }
    }
    let juncs = pu[hdr::JUNCS] as usize;
    let f_junc = pu[hdr::F_JUNC] as usize;
    for j in 0..pu[hdr::NJUNC] as usize {
        let vd =
            unsafe { work.node(s.x, pu[juncs + 5 * j]) - work.node(s.x, pu[juncs + 5 * j + 1]) };
        if vd > DIODE_EXP_ARG_MAX * pf[f_junc + 6 * j + 1] {
            return status::EXP_RANGE;
        }
    }
    status::CONVERGED
}

/// The growth test of `UnrolledSymbolicLu::factors_are_stable`, in plain floating point; `max_a = max |J|`.
#[inline(always)]
unsafe fn factors_are_stable(pu: &[u32], work: Work, s: Slots, max_a: f64) -> bool {
    let n = pu[hdr::N] as usize;
    let diag = pu[hdr::DIAG] as usize;
    let l_terms = pu[hdr::L_TERMS] as usize;
    let u_terms = pu[hdr::U_TERMS] as usize;
    let l_ptr = pu[hdr::L_PTR] as usize;
    let u_ptr = pu[hdr::U_PTR] as usize;
    unsafe {
        if max_a == 0.0 || n == 0 {
            return false;
        }
        let mut inv_min = f64::INFINITY;
        let mut inv_max = 0.0_f64;
        for k in 0..n {
            let b = work.ld(s.lu + pu[diag + k] as usize).abs();
            if !b.is_finite() {
                return false;
            }
            inv_min = inv_min.min(b);
            inv_max = inv_max.max(b);
        }
        if inv_min == 0.0 {
            return false;
        }
        let mut max_lu = 0.0_f64;
        for t in 0..pu[l_ptr + n] as usize {
            let v = work.ld(s.lu + pu[l_terms + 2 * t + 1] as usize).abs();
            if !v.is_finite() {
                return false;
            }
            max_lu = max_lu.max(v);
        }
        for t in 0..pu[u_ptr + n] as usize {
            let v = work.ld(s.lu + pu[u_terms + 2 * t + 1] as usize).abs();
            if !v.is_finite() {
                return false;
            }
            max_lu = max_lu.max(v);
        }
        let max_u_diag = 1.0 / inv_min;
        let min_piv = 1.0 / inv_max;
        max_lu.max(max_u_diag) <= MAX_FACTOR_GROWTH * max_a && min_piv >= 1e-12 * max_a
    }
}
