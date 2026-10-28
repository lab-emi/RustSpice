//! Batched DC operating points: one circuit topology solved at many parameter corners (Monte Carlo, PVT, supply
//! corners).
//!
//! [`BatchPlan`] flattens what the CPU solver uses for one DC Newton solve: the Jacobian / RHS gather tables, a
//! static-pivot elimination stream from its unrolled-LU compiler (on an order chosen to stay stable without
//! re-pivoting), the device models (plain diodes, level-1 MOSFETs, bulk junctions), the base values of every linear
//! device and the Newton tolerances. [`solve_cpu_ref`] runs the plan with rayon, one corner per task (the `cpu_ref`
//! implementation); [`CudaBatchSolver`] runs the same source compiled to PTX by cuda-oxide, one corner per CUDA
//! thread. Corners whose status is not [`status::CONVERGED`] are the caller's to re-solve on the CPU, whose solver
//! also has dynamic re-pivoting, gmin stepping and source stepping.

use std::sync::Arc;

use rayon::prelude::*;
use rustspice_backend_cpu::{SymbolicLuOp, UnrolledSymbolicLu};
use rustspice_core::{CompiledTopology, DeviceSoaBuffer};

use crate::driver::{CudaDriver, CudaError, CudaModule, DeviceBuffer, KernelArg};

/// The per-corner Newton solve, shared verbatim with the GPU kernel crate (`kernels/gpu`).
#[allow(dead_code)]
pub mod kernel {
    include!("../kernels/batch_newton.rs");
}

pub use kernel::status;

/// PTX of `kernels/batch_newton.rs` (rebuilt by `kernels/build_ptx.sh`).
pub const BATCH_NEWTON_PTX: &str = include_str!("../kernels/batch_newton.ptx");
const KERNEL_NAME: &str = "batch_newton_dc";
const THREADS_PER_BLOCK: u32 = 128;

/// Newton iteration limits and tolerances (the CPU solver's `NrOptions`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewtonSettings {
    pub max_iters: usize,
    pub reltol: f64,
    pub vntol: f64,
    pub abstol: f64,
    pub res_reltol: f64,
    pub box_release_iters: usize,
}

/// A device value that differs between corners.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamSlot {
    /// `cond_buf[index]` (a resistor's conductance).
    Conductance(u32),
    /// `rhs_buf[index]` (an independent source's DC value).
    SourceValue(u32),
    /// Saturation current of plain diode `index`.
    DiodeIs(u32),
    /// Emission coefficient of plain diode `index`.
    DiodeN(u32),
    /// Width (`mos_w`, multiplier included) of MOSFET `index`.
    MosW(u32),
    /// Effective channel length (`mos_l`) of MOSFET `index`.
    MosL(u32),
    /// `VTO` of MOSFET `index`.
    MosVto(u32),
    /// `KP` of MOSFET `index`.
    MosKp(u32),
    /// `LAMBDA` of MOSFET `index`.
    MosLambda(u32),
}

impl ParamSlot {
    /// The MOSFET and the index of its value in `hdr::MOS_PARAM` (`W`, `L`, `VTO`, `KP`, `LAMBDA`).
    fn mos_value(self) -> Option<(u32, usize)> {
        match self {
            ParamSlot::MosW(m) => Some((m, 0)),
            ParamSlot::MosL(m) => Some((m, 1)),
            ParamSlot::MosVto(m) => Some((m, 2)),
            ParamSlot::MosKp(m) => Some((m, 3)),
            ParamSlot::MosLambda(m) => Some((m, 4)),
            _ => None,
        }
    }
}

/// Flattened solve plan: a `u32` part (header, then index arrays) and an `f64` part (scalars, then value arrays).
#[derive(Clone, Debug)]
pub struct BatchPlan {
    pu: Vec<u32>,
    pf: Vec<f64>,
    dim: usize,
    nparam: usize,
    slots: usize,
}

/// Conductance slots that a device writes at every evaluation or that a parameter sets per corner.
fn dynamic_conductances(soa: &DeviceSoaBuffer, params: &[ParamSlot]) -> Vec<bool> {
    let mut dynamic = vec![false; soa.cond_buf.len()];
    let slots = soa
        .diode_cond_buf_idx
        .iter()
        .chain(&soa.mos_gm_buf_idx)
        .chain(&soa.mos_gds_buf_idx)
        .chain(&soa.mos_gmbs_buf_idx)
        .chain(&soa.mos_rev_gm_buf_idx)
        .chain(&soa.mos_rev_gmbs_buf_idx)
        .chain(soa.ext.junctions.iter().map(|j| &j.cond_buf_idx))
        .chain(&soa.ext.bulk.cond_buf_idx)
        .chain(params.iter().filter_map(|p| match p {
            ParamSlot::Conductance(i) => Some(i),
            _ => None,
        }));
    for &i in slots {
        if let Some(d) = dynamic.get_mut(i as usize) {
            *d = true;
        }
    }
    dynamic
}

impl BatchPlan {
    /// Jacobian entries of `topo` (in the DC state that [`Self::build`] expects) that are zero in every corner and
    /// every Newton iteration: no device writes them, no parameter sets them, and their static value is 0 (the
    /// structural diagonal of a voltage source branch, a capacitor at DC). Leaving them out of the LU avoids the fill
    /// that their structure alone would cause.
    pub fn constant_zero_entries(topo: &CompiledTopology, params: &[ParamSlot]) -> Vec<bool> {
        let soa = &topo.soa;
        let gt = &soa.gather_table;
        let nnz = topo.jacobian.values.len();
        if gt.nnz_ptr.len() != nnz + 1 {
            return vec![false; nnz];
        }
        let dynamic = dynamic_conductances(soa, params);
        let mut zero: Vec<bool> = (0..nnz)
            .map(|k| {
                let terms = &gt.nnz_terms[gt.nnz_ptr[k]..gt.nnz_ptr[k + 1]];
                terms.iter().all(|t| !dynamic[t.buffer_idx as usize])
                    && terms.iter().fold(0.0f64, |acc, t| {
                        acc + t.sign * soa.cond_buf[t.buffer_idx as usize]
                    }) == 0.0
            })
            .collect();
        for &k in &soa.dc_floating_diag_idx {
            if k < nnz {
                zero[k] = false;
            }
        }
        zero
    }

    /// Builds the plan of `topo`, which must hold the DC state at `x = 0` with the sources at their DC values
    /// (`evaluate_devices` after `update_sources_at_time(None)`): its `cond_buf` / `rhs_buf` are the base values.
    /// `perm`, `csc_value_map` and `lu` are an ordering and static-pivot LU of `topo.jacobian` (the solver's
    /// `AdaptiveSparseSolver::static_pivot_plan`), whose reordered entry `k_new` is `csc_value_map[k_new]` of the
    /// Jacobian; an entry it leaves out must be among [`Self::constant_zero_entries`].
    ///
    /// Supported: resistors, capacitors (open), inductors (short), independent sources, linear controlled
    /// sources, plain diodes and level-1 MOSFETs with their bulk junctions and Meyer capacitances (open at DC),
    /// with at least one nonlinear device. Anything else is an `Err` naming the reason.
    pub fn build(
        topo: &CompiledTopology,
        perm: &[usize],
        csc_value_map: &[usize],
        lu: &UnrolledSymbolicLu,
        newton: &NewtonSettings,
        params: &[ParamSlot],
    ) -> Result<Self, String> {
        let soa = &topo.soa;
        let jac = &topo.jacobian;
        let n = soa.mna_dim;
        let nnz = jac.values.len();
        let ext = &soa.ext;
        if let Some(i) = soa.mos_level.iter().position(|&l| l != 1) {
            return Err(format!(
                "MOSFET `{}` is level {} (the kernel has level 1)",
                soa.mos_names[i], soa.mos_level[i]
            ));
        }
        if !ext.poly.is_empty() {
            return Err("polynomial controlled sources are not supported".into());
        }
        // a full junction is evaluated as a plain one when it differs from it only by its charge (none at DC)
        if let Some(j) = ext
            .junctions
            .iter()
            .find(|j| j.cubic_reverse || j.xbv.is_finite() || j.ikf.is_finite() || j.isr != 0.0)
        {
            return Err(format!(
                "junction `{}` has a reverse law, breakdown, knee or recombination current",
                j.name
            ));
        }
        let nmos = soa.mos_names.len();
        let njunc = ext.junctions.len() + ext.bulk.len();
        if soa.diode_names.is_empty() && nmos == 0 && njunc == 0 {
            return Err("linear circuit (one direct solve per corner on the CPU)".into());
        }
        if !soa.nodeset_nodes.is_empty() {
            return Err(".nodeset is not supported".into());
        }
        if soa.is_transient || soa.gmin != soa.gmin_nominal {
            return Err("the topology is not in its DC state".into());
        }
        if lu.dim != n || perm.len() != n || csc_value_map.len() > nnz || jac.ncols != n {
            return Err("the LU plan does not belong to this Jacobian".into());
        }
        let gt = &soa.gather_table;
        if gt.nnz_ptr.len() != nnz + 1 || gt.rhs_ptr.len() != n + 1 {
            return Err("the gather table does not belong to this Jacobian".into());
        }
        let ndiode = soa.diode_names.len();
        for p in params {
            let ok = match *p {
                ParamSlot::Conductance(i) => (i as usize) < soa.cond_buf.len(),
                ParamSlot::SourceValue(i) => (i as usize) < soa.rhs_buf.len(),
                ParamSlot::DiodeIs(d) | ParamSlot::DiodeN(d) => (d as usize) < ndiode,
                ParamSlot::MosW(m)
                | ParamSlot::MosL(m)
                | ParamSlot::MosVto(m)
                | ParamSlot::MosKp(m)
                | ParamSlot::MosLambda(m) => (m as usize) < nmos,
            };
            if !ok {
                return Err(format!("parameter slot {p:?} is out of range"));
            }
        }

        // dynamic values: written by a diode, a MOSFET or a junction, or set by a parameter; every other device
        // value is the same in all corners and is summed into the plan
        const STATIC: u32 = u32::MAX;
        let mut dcond = vec![STATIC; soa.cond_buf.len()];
        let mut drhs = vec![STATIC; soa.rhs_buf.len()];
        let (mut dcond0, mut drhs0) = (Vec::new(), Vec::new());
        let mark = |map: &mut [u32], base: &mut Vec<f64>, src: &[f64], i: usize| -> u32 {
            if map[i] == STATIC {
                map[i] = base.len() as u32;
                base.push(src[i]);
            }
            map[i]
        };
        let diode_slots: Vec<(u32, u32)> = (0..ndiode)
            .map(|d| {
                (
                    mark(
                        &mut dcond,
                        &mut dcond0,
                        &soa.cond_buf,
                        soa.diode_cond_buf_idx[d] as usize,
                    ),
                    mark(
                        &mut drhs,
                        &mut drhs0,
                        &soa.rhs_buf,
                        soa.diode_rhs_buf_idx[d] as usize,
                    ),
                )
            })
            .collect();
        let mos_slots: Vec<[u32; 6]> = (0..nmos)
            .map(|m| {
                let mut c = |i: u32| mark(&mut dcond, &mut dcond0, &soa.cond_buf, i as usize);
                [
                    c(soa.mos_gm_buf_idx[m]),
                    c(soa.mos_gds_buf_idx[m]),
                    c(soa.mos_gmbs_buf_idx[m]),
                    c(soa.mos_rev_gm_buf_idx[m]),
                    c(soa.mos_rev_gmbs_buf_idx[m]),
                    mark(
                        &mut drhs,
                        &mut drhs0,
                        &soa.rhs_buf,
                        soa.mos_rhs_buf_idx[m] as usize,
                    ),
                ]
            })
            .collect();
        // plain junctions in the CPU's order (full ones, then the bulk table): anode, cathode, conductance slot,
        // RHS slot, folded; is, n Vt, fold sign
        let mut junc_u: Vec<u32> = Vec::with_capacity(5 * njunc);
        let mut junc_f: Vec<f64> = Vec::with_capacity(6 * njunc);
        let mut push_junction = |anode: usize,
                                 cathode: usize,
                                 cond: u32,
                                 rhs: u32,
                                 is: f64,
                                 nvt: f64,
                                 vcrit: f64,
                                 fold_sign: f64| {
            junc_u.extend([
                anode as u32,
                cathode as u32,
                mark(&mut dcond, &mut dcond0, &soa.cond_buf, cond as usize),
                mark(&mut drhs, &mut drhs0, &soa.rhs_buf, rhs as usize),
                u32::from(fold_sign != 0.0),
            ]);
            junc_f.extend([is, nvt, 1.0 / nvt, is / nvt, vcrit, fold_sign]);
        };
        for j in &ext.junctions {
            let sign = if j.fold_mos.is_some() {
                j.fold_sign
            } else {
                0.0
            };
            push_junction(
                j.anode,
                j.cathode,
                j.cond_buf_idx,
                j.rhs_buf_idx,
                j.is,
                j.nvt,
                j.vcrit,
                sign,
            );
        }
        let b = &ext.bulk;
        for k in 0..b.len() {
            push_junction(
                b.anode[k],
                b.cathode[k],
                b.cond_buf_idx[k],
                b.rhs_buf_idx[k],
                b.is[k],
                b.nvt[k],
                b.vcrit[k],
                b.fold_sign[k],
            );
        }
        // the corner's value of a MOSFET parameter is read where the MOSFET is evaluated: 1 + its slot
        let mut mos_param = vec![0u32; 5 * nmos];
        for (j, p) in params.iter().enumerate() {
            if let Some((m, k)) = p.mos_value() {
                mos_param[5 * m as usize + k] = j as u32 + 1;
            }
        }
        let param_words: Vec<[u32; 2]> = params
            .iter()
            .map(|p| match *p {
                ParamSlot::Conductance(i) => [
                    kernel::param::COND,
                    mark(&mut dcond, &mut dcond0, &soa.cond_buf, i as usize),
                ],
                ParamSlot::SourceValue(i) => [
                    kernel::param::RHSB,
                    mark(&mut drhs, &mut drhs0, &soa.rhs_buf, i as usize),
                ],
                ParamSlot::DiodeIs(d) => [kernel::param::DIODE_IS, d],
                ParamSlot::DiodeN(d) => [kernel::param::DIODE_N, d],
                ParamSlot::MosW(m) => [kernel::param::MOS_W, m],
                ParamSlot::MosL(m) => [kernel::param::MOS_L, m],
                ParamSlot::MosVto(m) => [kernel::param::MOS_VTO, m],
                ParamSlot::MosKp(m) => [kernel::param::MOS_KP, m],
                ParamSlot::MosLambda(m) => [kernel::param::MOS_LAMBDA, m],
            })
            .collect();

        // the factors read the reordered matrix: non-zero `k_new` of it is `csc_value_map[k_new]` of ours; the
        // plan's Jacobian is the entries the factors read, in CSC order
        const DROPPED: u32 = u32::MAX;
        let mut lu_of = vec![DROPPED; nnz];
        for (k_new, &k) in csc_value_map.iter().enumerate() {
            if k >= nnz || lu_of[k] != DROPPED {
                return Err("the LU plan does not belong to this Jacobian".into());
            }
            lu_of[k] = lu.csc_to_lu[k_new];
        }
        let kept: Vec<usize> = (0..nnz).filter(|&k| lu_of[k] != DROPPED).collect();
        let csc_to_lu: Vec<u32> = kept.iter().map(|&k| lu_of[k]).collect();
        let mut is_gathered = vec![false; lu.lu_nnz];
        for &l in &csc_to_lu {
            is_gathered[l as usize] = true;
        }
        let fill: Vec<u32> = (0..lu.lu_nnz as u32)
            .filter(|&l| !is_gathered[l as usize])
            .collect();

        // Jacobian: static sums (table order, then the floating-node conductance as the CPU adds it) and dynamic
        // terms
        let mut floating = vec![false; nnz];
        for &k in &soa.dc_floating_diag_idx {
            if k < nnz {
                floating[k] = true;
            }
        }
        let mut jac_static = Vec::with_capacity(kept.len());
        let (mut jac_dptr, mut jac_dsrc, mut jac_dsign) = (vec![0u32], Vec::new(), Vec::new());
        for k in 0..nnz {
            let mut acc = 0.0f64;
            let mut dynamic = false;
            for t in &gt.nnz_terms[gt.nnz_ptr[k]..gt.nnz_ptr[k + 1]] {
                let b = t.buffer_idx as usize;
                if dcond[b] == STATIC {
                    acc += t.sign * soa.cond_buf[b];
                } else {
                    dynamic = true;
                    if lu_of[k] != DROPPED {
                        jac_dsrc.push(dcond[b]);
                        jac_dsign.push(t.sign);
                    }
                }
            }
            if floating[k] {
                acc += 1e-12;
            }
            if lu_of[k] == DROPPED {
                if dynamic || acc != 0.0 {
                    return Err(format!(
                        "the LU plan leaves out Jacobian entry {k}, which is not constantly zero"
                    ));
                }
                continue;
            }
            jac_static.push(acc);
            jac_dptr.push(jac_dsrc.len() as u32);
        }
        let mut rhs_static = vec![0.0f64; n];
        let (mut rhs_dptr, mut rhs_dsrc, mut rhs_dsign) = (vec![0u32], Vec::new(), Vec::new());
        for (i, rs) in rhs_static.iter_mut().enumerate() {
            let mut acc = 0.0f64;
            for t in &gt.rhs_terms[gt.rhs_ptr[i]..gt.rhs_ptr[i + 1]] {
                let b = t.buffer_idx as usize;
                if drhs[b] == STATIC {
                    acc += t.sign * soa.rhs_buf[b];
                } else {
                    rhs_dsrc.push(drhs[b]);
                    rhs_dsign.push(t.sign);
                }
            }
            *rs = acc;
            rhs_dptr.push(rhs_dsrc.len() as u32);
        }

        // rows of the Jacobian with their columns ascending (the order of the CPU's column sweep)
        let mut row_count = vec![0u32; n + 1];
        for &k in &kept {
            row_count[jac.row_idx[k] + 1] += 1;
        }
        for i in 0..n {
            row_count[i + 1] += row_count[i];
        }
        let row_ptr = row_count.clone();
        let mut next = row_count;
        let (mut row_col, mut row_lu) = (vec![0u32; kept.len()], vec![0u32; kept.len()]);
        for col in 0..n {
            let span = jac.col_ptr[col]..jac.col_ptr[col + 1];
            for (&r, &l) in jac.row_idx[span.clone()].iter().zip(&lu_of[span]) {
                if l == DROPPED {
                    continue;
                }
                let e = next[r] as usize;
                row_col[e] = col as u32;
                row_lu[e] = l;
                next[r] += 1;
            }
        }
        // right-hand side position k of the factors reads residual row perm[row_perm[k]]
        let mut row_pos = vec![0u32; n];
        for (k, &r) in lu.row_perm.iter().enumerate() {
            row_pos[perm[r]] = k as u32;
        }
        let mut x_perm = vec![0u32; n];
        for (new_i, &old_i) in perm.iter().enumerate() {
            x_perm[old_i] = new_i as u32;
        }

        use kernel::hdr;
        let mut pu = vec![0u32; hdr::LEN];
        let mut pf = vec![0.0f64; kernel::fhdr::LEN];
        fn push_u(pu: &mut Vec<u32>, slot: usize, data: impl IntoIterator<Item = u32>) {
            pu[slot] = pu.len() as u32;
            pu.extend(data);
        }
        fn push_f(
            pu: &mut [u32],
            pf: &mut Vec<f64>,
            slot: usize,
            data: impl IntoIterator<Item = f64>,
        ) {
            pu[slot] = pf.len() as u32;
            pf.extend(data);
        }
        pu[hdr::N] = n as u32;
        pu[hdr::NUM_NODES] = soa.num_nodes as u32;
        pu[hdr::NNZ] = kept.len() as u32;
        pu[hdr::LU_NNZ] = lu.lu_nnz as u32;
        pu[hdr::NDCOND] = dcond0.len() as u32;
        pu[hdr::NDRHS] = drhs0.len() as u32;
        pu[hdr::NDIODE] = ndiode as u32;
        pu[hdr::NOPS] = lu.factor_ops.len() as u32;
        pu[hdr::NPARAM] = params.len() as u32;
        pu[hdr::MAX_ITERS] = newton.max_iters as u32;
        pu[hdr::NVSRC] = soa.vsrc_names.len() as u32;
        pu[hdr::NFILL] = fill.len() as u32;
        pu[hdr::BOX_TIGHT] = u32::from(soa.isrc_names.is_empty() && soa.vccs_names.is_empty());
        pu[hdr::BOX_RELEASE] = newton.box_release_iters as u32;
        pu[hdr::NMOS] = nmos as u32;
        pu[hdr::NJUNC] = njunc as u32;

        push_u(&mut pu, hdr::ROW_PTR, row_ptr);
        push_u(&mut pu, hdr::ROW_COL, row_col);
        push_u(&mut pu, hdr::ROW_LU, row_lu);
        push_u(&mut pu, hdr::JAC_DPTR, jac_dptr);
        push_u(&mut pu, hdr::JAC_DSRC, jac_dsrc);
        push_u(&mut pu, hdr::JAC_LU, csc_to_lu);
        push_u(&mut pu, hdr::RHS_DPTR, rhs_dptr);
        push_u(&mut pu, hdr::RHS_DSRC, rhs_dsrc);
        push_u(
            &mut pu,
            hdr::OPS,
            lu.factor_ops.iter().flat_map(|op| match *op {
                SymbolicLuOp::InvertPivot { diag_idx } => {
                    [kernel::op::INVERT_PIVOT, diag_idx, 0, 0]
                }
                SymbolicLuOp::ScaleMultiplier {
                    lower_idx,
                    diag_idx,
                } => [kernel::op::SCALE_MULTIPLIER, lower_idx, diag_idx, 0],
                SymbolicLuOp::Eliminate {
                    target_idx,
                    lower_idx,
                    upper_idx,
                } => [kernel::op::ELIMINATE, target_idx, lower_idx, upper_idx],
            }),
        );
        push_u(&mut pu, hdr::DIAG, lu.diag_lu_idx.iter().copied());
        push_u(&mut pu, hdr::L_PTR, lu.l_col_ptr.iter().map(|&v| v as u32));
        push_u(
            &mut pu,
            hdr::L_TERMS,
            lu.l_terms.iter().flat_map(|t| [t.vec_idx, t.lu_idx]),
        );
        push_u(&mut pu, hdr::U_PTR, lu.u_col_ptr.iter().map(|&v| v as u32));
        push_u(
            &mut pu,
            hdr::U_TERMS,
            lu.u_terms.iter().flat_map(|t| [t.vec_idx, t.lu_idx]),
        );
        push_u(&mut pu, hdr::FILL, fill);
        push_u(&mut pu, hdr::ROW_POS, row_pos);
        push_u(&mut pu, hdr::X_PERM, x_perm);
        push_u(
            &mut pu,
            hdr::DIODES,
            (0..ndiode).flat_map(|d| {
                [
                    soa.diode_anode[d] as u32,
                    soa.diode_cathode[d] as u32,
                    diode_slots[d].0,
                    diode_slots[d].1,
                ]
            }),
        );
        push_u(&mut pu, hdr::PARAMS, param_words.into_iter().flatten());
        push_u(
            &mut pu,
            hdr::MOS,
            (0..nmos).flat_map(|m| {
                let s = mos_slots[m];
                [
                    soa.mos_drain[m] as u32,
                    soa.mos_gate[m] as u32,
                    soa.mos_source[m] as u32,
                    soa.mos_bulk[m] as u32,
                    s[0],
                    s[1],
                    s[2],
                    s[3],
                    s[4],
                    s[5],
                ]
            }),
        );
        push_u(&mut pu, hdr::MOS_PARAM, mos_param);
        push_u(&mut pu, hdr::JUNCS, junc_u);
        push_u(
            &mut pu,
            hdr::VSRC_DYN,
            soa.vsrc_rhs_buf_idx
                .iter()
                .map(|&i| match drhs[i as usize] {
                    STATIC => 0,
                    s => s + 1,
                }),
        );

        pf[kernel::fhdr::VT] = soa.temp_vt;
        pf[kernel::fhdr::GMIN] = soa.gmin;
        pf[kernel::fhdr::RELTOL] = newton.reltol;
        pf[kernel::fhdr::VNTOL] = newton.vntol;
        pf[kernel::fhdr::ABSTOL] = newton.abstol;
        pf[kernel::fhdr::RES_RELTOL] = newton.res_reltol;
        push_f(&mut pu, &mut pf, hdr::F_JAC_STATIC, jac_static);
        push_f(&mut pu, &mut pf, hdr::F_JAC_DSIGN, jac_dsign);
        push_f(&mut pu, &mut pf, hdr::F_RHS_STATIC, rhs_static);
        push_f(&mut pu, &mut pf, hdr::F_RHS_DSIGN, rhs_dsign);
        push_f(&mut pu, &mut pf, hdr::F_DCOND0, dcond0);
        push_f(&mut pu, &mut pf, hdr::F_DRHS0, drhs0);
        push_f(
            &mut pu,
            &mut pf,
            hdr::F_DIODE_IS,
            soa.diode_is.iter().copied(),
        );
        push_f(
            &mut pu,
            &mut pf,
            hdr::F_DIODE_N,
            soa.diode_n.iter().copied(),
        );
        push_f(
            &mut pu,
            &mut pf,
            hdr::F_MOS,
            (0..nmos).flat_map(|m| {
                [
                    soa.mos_polarity[m],
                    soa.mos_kp[m],
                    soa.mos_w[m],
                    soa.mos_l[m],
                    soa.mos_vto[m],
                    soa.mos_gamma[m],
                    soa.mos_phi[m],
                    soa.mos_lambda[m],
                ]
            }),
        );
        push_f(&mut pu, &mut pf, hdr::F_JUNC, junc_f);
        push_f(
            &mut pu,
            &mut pf,
            hdr::F_VSRC,
            soa.vsrc_rhs_buf_idx
                .iter()
                .map(|&i| soa.rhs_buf[i as usize]),
        );

        if pu.len() > u32::MAX as usize || pf.len() > u32::MAX as usize {
            return Err("plan too large for 32-bit offsets".into());
        }
        let slots = kernel::workspace_slots(&pu);
        Ok(Self {
            pu,
            pf,
            dim: n,
            nparam: params.len(),
            slots,
        })
    }

    /// MNA dimension (length of each corner's solution).
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Number of parameter slots per corner.
    pub fn num_params(&self) -> usize {
        self.nparam
    }

    /// `f64` workspace words of one corner.
    pub fn workspace_slots(&self) -> usize {
        self.slots
    }

    /// Size of the plan itself in bytes.
    pub fn plan_bytes(&self) -> usize {
        4 * self.pu.len() + 8 * self.pf.len()
    }

    /// Sizes of the plan: dimension, Jacobian non-zeros, LU entries (fill-in), elimination operations, triangular
    /// terms, diodes, MOSFETs, junctions, and the per-corner values with the gather terms that read them.
    pub fn summary(&self) -> String {
        use kernel::hdr;
        let h = |k: usize| self.pu[k] as usize;
        let nnz = h(hdr::NNZ);
        let n = h(hdr::N);
        format!(
            "n {n}, nnz {nnz}, LU entries {} ({} fill-in), elimination ops {}, L terms {}, U terms {}, diodes {}, MOSFETs {}, junctions {}, per-corner conductances {} (Jacobian terms {}), per-corner RHS values {} (RHS terms {}), workspace {} words",
            h(hdr::LU_NNZ),
            h(hdr::NFILL),
            h(hdr::NOPS),
            self.pu[h(hdr::L_PTR) + n],
            self.pu[h(hdr::U_PTR) + n],
            h(hdr::NDIODE),
            h(hdr::NMOS),
            h(hdr::NJUNC),
            h(hdr::NDCOND),
            self.pu[h(hdr::JAC_DPTR) + nnz],
            h(hdr::NDRHS),
            self.pu[h(hdr::RHS_DPTR) + n],
            self.slots,
        )
    }

    fn check_params(&self, params: &[f64], corners: usize) {
        assert_eq!(
            params.len(),
            self.nparam * corners,
            "params must hold num_params() values per corner"
        );
    }
}

/// Solutions of a batch, corner after corner.
#[derive(Clone, Debug, Default)]
pub struct BatchSolution {
    pub dim: usize,
    /// `x[c * dim..(c + 1) * dim]` is corner `c`'s solution (meaningful only when it converged).
    pub x: Vec<f64>,
    /// [`status`] of each corner.
    pub status: Vec<u32>,
    /// Newton iterations of each corner.
    pub iterations: Vec<u32>,
}

impl BatchSolution {
    /// Solution of corner `c`.
    pub fn corner(&self, c: usize) -> &[f64] {
        &self.x[c * self.dim..(c + 1) * self.dim]
    }

    /// Corners whose status is not [`status::CONVERGED`].
    pub fn unconverged(&self) -> impl Iterator<Item = usize> + '_ {
        self.status
            .iter()
            .enumerate()
            .filter(|(_, &s)| s != status::CONVERGED)
            .map(|(c, _)| c)
    }
}

/// `cpu_ref`: solves every corner on the CPU with rayon. `params[j * corners + c]` is slot `j` of corner `c`.
pub fn solve_cpu_ref(plan: &BatchPlan, params: &[f64], corners: usize) -> BatchSolution {
    plan.check_params(params, corners);
    const BLOCK: usize = 16;
    let n = plan.dim;
    let mut out = BatchSolution {
        dim: n,
        x: vec![0.0; n * corners],
        status: vec![0; corners],
        iterations: vec![0; corners],
    };
    out.x
        .par_chunks_mut(n * BLOCK)
        .zip(out.status.par_chunks_mut(BLOCK))
        .zip(out.iterations.par_chunks_mut(BLOCK))
        .enumerate()
        .for_each(|(b, ((xb, sb), ib))| {
            let mut w = vec![0.0f64; plan.slots];
            for k in 0..sb.len() {
                let work = kernel::Work {
                    w: w.as_mut_ptr(),
                    stride: 1,
                    corner: 0,
                };
                // SAFETY: `w` holds the plan's workspace for one corner (stride 1); `params` was checked above.
                let (st, it) = unsafe {
                    kernel::solve_corner(&plan.pu, &plan.pf, params, corners, b * BLOCK + k, work)
                };
                sb[k] = st;
                ib[k] = it;
                xb[k * n..(k + 1) * n].copy_from_slice(&w[..n]);
            }
        });
    out
}

/// The batched kernel loaded on one CUDA device.
pub struct CudaBatchSolver {
    driver: Arc<CudaDriver>,
    module: CudaModule,
    /// Device memory budget per launch when [`Self::solve`] gets none (`None`: half the free memory, at most 2 GiB).
    pub launch_budget: Option<usize>,
}

impl CudaBatchSolver {
    /// Loads the driver, retains device `ordinal` and JIT-compiles the embedded PTX for it.
    pub fn new(ordinal: usize) -> Result<Self, CudaError> {
        Self::with_max_registers(ordinal, Self::DEFAULT_MAX_REGISTERS)
    }

    /// Register cap of [`Self::new`].
    pub const DEFAULT_MAX_REGISTERS: Option<u32> = None;

    /// [`Self::new`] with an explicit register cap per thread (`None`: the compiler's choice).
    pub fn with_max_registers(
        ordinal: usize,
        max_registers: Option<u32>,
    ) -> Result<Self, CudaError> {
        let driver = CudaDriver::load(ordinal)?;
        let module = driver.load_ptx(BATCH_NEWTON_PTX, max_registers)?;
        Ok(Self {
            driver,
            module,
            launch_budget: None,
        })
    }

    /// Device name.
    pub fn device_name(&self) -> &str {
        &self.driver.device_name
    }

    /// CUDA version of the driver as `major.minor`.
    pub fn driver_version(&self) -> String {
        let v = self.driver.driver_version;
        format!("{}.{}", v / 1000, (v % 1000) / 10)
    }

    /// Corners per launch so that the per-launch device memory stays under `budget_bytes`.
    pub fn corners_per_launch(plan: &BatchPlan, corners: usize, budget_bytes: usize) -> usize {
        let per_corner = 8 * (plan.slots + plan.nparam) + 8;
        let k = (budget_bytes / per_corner.max(1)).max(THREADS_PER_BLOCK as usize);
        k.min(corners.max(1))
    }

    /// Solves every corner on the GPU, `params` as for [`solve_cpu_ref`]. Corners run in launches of at most
    /// [`Self::corners_per_launch`] corners, with a device memory budget of `budget_bytes` (default:
    /// [`Self::launch_budget`]).
    pub fn solve(
        &self,
        plan: &BatchPlan,
        params: &[f64],
        corners: usize,
        budget_bytes: Option<usize>,
    ) -> Result<BatchSolution, CudaError> {
        plan.check_params(params, corners);
        let n = plan.dim;
        let mut out = BatchSolution {
            dim: n,
            x: vec![0.0; n * corners],
            status: vec![0; corners],
            iterations: vec![0; corners],
        };
        if corners == 0 {
            return Ok(out);
        }
        let budget = match budget_bytes.or(self.launch_budget) {
            Some(b) => b,
            None => (self.driver.mem_info()?.0 / 2).min(2 << 30),
        };
        let chunk = Self::corners_per_launch(plan, corners, budget);
        if chunk > u32::MAX as usize {
            return Err(CudaError("too many corners per launch".into()));
        }
        let d = &self.driver;
        let pu: DeviceBuffer<u32> = d.upload(&plan.pu)?;
        let pf: DeviceBuffer<f64> = d.upload(&plan.pf)?;
        let work: DeviceBuffer<f64> = d.alloc(chunk * plan.slots)?;
        let pdev: DeviceBuffer<f64> = d.alloc(chunk * plan.nparam)?;
        let odev: DeviceBuffer<u32> = d.alloc(2 * chunk)?;
        let mut chunk_params = vec![0.0f64; chunk * plan.nparam];
        let mut xs = vec![0.0f64; chunk * n];
        let mut so = vec![0u32; 2 * chunk];
        let mut c0 = 0;
        while c0 < corners {
            let k = chunk.min(corners - c0);
            // slot-major with stride k, as the kernel indexes it
            for j in 0..plan.nparam {
                chunk_params[j * k..(j + 1) * k]
                    .copy_from_slice(&params[j * corners + c0..j * corners + c0 + k]);
            }
            pdev.copy_from(&chunk_params[..k * plan.nparam])?;
            let args = [
                KernelArg::U64(pu.device_ptr()),
                KernelArg::U64(pu.len() as u64),
                KernelArg::U64(pf.device_ptr()),
                KernelArg::U64(pf.len() as u64),
                KernelArg::U64(pdev.device_ptr()),
                KernelArg::U64((k * plan.nparam) as u64),
                KernelArg::U32(k as u32),
                KernelArg::U64(work.device_ptr()),
                KernelArg::U64(odev.device_ptr()),
            ];
            let blocks = k.div_ceil(THREADS_PER_BLOCK as usize) as u32;
            // SAFETY: the arguments follow `batch_newton_dc(pu, pf, params, corners, work, out)`; `work` holds
            // `slots * chunk >= slots * k` words, `pdev` `k * nparam`, `odev` `2 * chunk`.
            unsafe {
                self.module
                    .launch(KERNEL_NAME, blocks, THREADS_PER_BLOCK, 0, &args)?;
            }
            // the `x` slots are the first `n` slots: words `0..n * k` of the workspace
            work.copy_to(&mut xs[..n * k])?;
            odev.copy_to(&mut so[..2 * k])?;
            for c in 0..k {
                for i in 0..n {
                    out.x[(c0 + c) * n + i] = xs[i * k + c];
                }
                out.status[c0 + c] = so[2 * c];
                out.iterations[c0 + c] = so[2 * c + 1];
            }
            c0 += k;
        }
        Ok(out)
    }
}
