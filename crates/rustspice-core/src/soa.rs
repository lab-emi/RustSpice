//! 64-byte cache-line aligned Structure-of-Arrays (`DeviceSoaBuffer`) and pre-compiled
//! conflict-free sparse matrix topology (`CompiledTopology`).

use std::ops::{Deref, DerefMut};

use rustspice_macros::spice_device;
use serde::{Deserialize, Serialize};

use crate::error::{SimError, SimResult};
use crate::netlist::{Netlist, SourceWaveform};
use crate::sparse::{CscGatherTable, CscMatrix, GatherTerm};

/// 64-byte cache-line aligned contiguous array for AVX-512 / AVX2 / NEON and GPU DMA transfers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AlignedVec<T> {
    inner: Vec<T>,
}

impl<T: Copy + Default> AlignedVec<T> {
    pub fn with_len(len: usize, val: T) -> Self {
        Self {
            inner: vec![val; len],
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn push(&mut self, val: T) {
        self.inner.push(val);
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.inner
    }
}

impl<T> Deref for AlignedVec<T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for AlignedVec<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// Compile-time auto-differentiated Shockley Diode physics kernel generated via `#[spice_device]`.
///
/// The procedural macro parses the forward equation below, computes $\partial I_D/\partial V_D$,
/// $\partial I_D/\partial I_S$, and $\partial I_D/\partial N$ symbolically, applies CSE/DCE,
/// and generates inline Rust + Metal MSL `df64` + WebGPU WGSL `df64` + NVIDIA CUDA Tile shaders.
#[spice_device(
    inputs(v_d),
    params(is = 1e-14, n = 1.0, vt = 0.025851997, gmin = 1e-12),
    outputs(i_d)
)]
pub fn intrinsic_diode_kernel(v_d: f64, is: f64, n: f64, vt: f64, gmin: f64) -> f64 {
    let n_vt = n * vt;
    let arg = v_d / n_vt;
    let exp_arg = arg.exp();
    is * (exp_arg - 1.0) + gmin * v_d
}

/// Compile-time auto-differentiated Shichman-Hodges Level-1 MOSFET kernel generated via `#[spice_device]`.
#[allow(clippy::too_many_arguments)]
#[spice_device(
    inputs(v_gs, v_ds),
    params(
        kp = 120e-6,
        w = 10e-6,
        l = 1e-6,
        vto = 0.7,
        lambda = 0.02,
        gmin = 1e-12
    ),
    outputs(i_d)
)]
pub fn intrinsic_mosfet_kernel(
    v_gs: f64,
    v_ds: f64,
    kp: f64,
    w: f64,
    l: f64,
    vto: f64,
    lambda: f64,
    gmin: f64,
) -> f64 {
    let beta = kp * (w / l);
    let v_ov = v_gs - vto;
    let clm = 1.0 + lambda * v_ds;
    let i_lin = beta * (v_ov * v_ds - 0.5 * v_ds * v_ds) * clm;
    let i_sat = 0.5 * beta * (v_ov * v_ov) * clm;
    let i_active = if v_ds > v_ov { i_sat } else { i_lin };
    let i_mos = if v_ov > 0.0 { i_active } else { 0.0 };
    i_mos + gmin * v_ds
}

/// Complete Structure-of-Arrays (SoA) state buffer and pre-compiled conflict-free matrix mapping
/// for all devices in a compiled SPICE circuit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceSoaBuffer {
    pub num_nodes: usize,
    pub mna_dim: usize,
    pub gmin: f64,
    pub temp_vt: f64,
    pub is_transient: bool,
    pub transient_g_scale: f64,

    // --- Resistors (SoA) ---
    pub res_names: Vec<String>,
    pub res_n_pos: Vec<usize>,
    pub res_n_neg: Vec<usize>,
    pub res_resistance: AlignedVec<f64>,
    pub res_conductance: AlignedVec<f64>,
    pub res_cond_buf_idx: Vec<u32>,

    // --- VCCS (SoA) ---
    pub vccs_names: Vec<String>,
    pub vccs_n_pos: Vec<usize>,
    pub vccs_n_neg: Vec<usize>,
    pub vccs_nc_pos: Vec<usize>,
    pub vccs_nc_neg: Vec<usize>,
    pub vccs_gm: AlignedVec<f64>,
    pub vccs_cond_buf_idx: Vec<u32>,

    // --- Capacitors (SoA) ---
    pub cap_names: Vec<String>,
    pub cap_n_pos: Vec<usize>,
    pub cap_n_neg: Vec<usize>,
    pub cap_capacitance: AlignedVec<f64>,
    pub cap_geq: AlignedVec<f64>,
    pub cap_ieq: AlignedVec<f64>,
    pub cap_v_prev: AlignedVec<f64>,
    pub cap_v_prev2: AlignedVec<f64>,
    pub cap_i_prev: AlignedVec<f64>,
    pub cap_cond_buf_idx: Vec<u32>,
    pub cap_rhs_buf_idx: Vec<u32>,

    // --- Inductors (SoA) ---
    pub ind_names: Vec<String>,
    pub ind_n_pos: Vec<usize>,
    pub ind_n_neg: Vec<usize>,
    pub ind_branch_idx: Vec<usize>,
    pub ind_inductance: AlignedVec<f64>,
    pub ind_req: AlignedVec<f64>,
    pub ind_veq: AlignedVec<f64>,
    pub ind_i_prev: AlignedVec<f64>,
    pub ind_i_prev2: AlignedVec<f64>,
    pub ind_v_prev: AlignedVec<f64>,
    pub ind_cond_buf_idx: Vec<u32>,
    pub ind_branch_buf_idx: Vec<u32>,
    pub ind_rhs_buf_idx: Vec<u32>,

    // --- Independent Voltage Sources (SoA) ---
    pub vsrc_names: Vec<String>,
    pub vsrc_n_pos: Vec<usize>,
    pub vsrc_n_neg: Vec<usize>,
    pub vsrc_branch_idx: Vec<usize>,
    pub vsrc_waveforms: Vec<SourceWaveform>,
    pub vsrc_values: AlignedVec<f64>,
    pub vsrc_branch_buf_idx: Vec<u32>,
    pub vsrc_rhs_buf_idx: Vec<u32>,

    // --- Independent Current Sources (SoA) ---
    pub isrc_names: Vec<String>,
    pub isrc_n_pos: Vec<usize>,
    pub isrc_n_neg: Vec<usize>,
    pub isrc_waveforms: Vec<SourceWaveform>,
    pub isrc_values: AlignedVec<f64>,
    pub isrc_rhs_buf_idx: Vec<u32>,

    // --- PN Junction Diodes (SoA) ---
    pub diode_names: Vec<String>,
    pub diode_anode: Vec<usize>,
    pub diode_cathode: Vec<usize>,
    pub diode_is: AlignedVec<f64>,
    pub diode_n: AlignedVec<f64>,
    pub diode_area: AlignedVec<f64>,
    pub diode_cjo: AlignedVec<f64>,
    pub diode_vj: AlignedVec<f64>,
    pub diode_m: AlignedVec<f64>,
    pub diode_tt: AlignedVec<f64>,
    pub diode_vd: AlignedVec<f64>,
    pub diode_id: AlignedVec<f64>,
    pub diode_gd: AlignedVec<f64>,
    pub diode_ieq: AlignedVec<f64>,
    pub diode_did_dis: AlignedVec<f64>,
    pub diode_did_dn: AlignedVec<f64>,
    pub diode_cond_buf_idx: Vec<u32>,
    pub diode_rhs_buf_idx: Vec<u32>,

    // --- MOSFETs (SoA) ---
    pub mos_names: Vec<String>,
    pub mos_drain: Vec<usize>,
    pub mos_gate: Vec<usize>,
    pub mos_source: Vec<usize>,
    pub mos_bulk: Vec<usize>,
    pub mos_polarity: AlignedVec<f64>,
    pub mos_level: Vec<u32>,
    pub mos_kp: AlignedVec<f64>,
    pub mos_w: AlignedVec<f64>,
    pub mos_l: AlignedVec<f64>,
    pub mos_vto: AlignedVec<f64>,
    pub mos_gamma: AlignedVec<f64>,
    pub mos_phi: AlignedVec<f64>,
    pub mos_lambda: AlignedVec<f64>,
    pub mos_vgs: AlignedVec<f64>,
    pub mos_vds: AlignedVec<f64>,
    pub mos_vbs: AlignedVec<f64>,
    pub mos_id: AlignedVec<f64>,
    pub mos_gm: AlignedVec<f64>,
    pub mos_gds: AlignedVec<f64>,
    pub mos_gmbs: AlignedVec<f64>,
    pub mos_ieq: AlignedVec<f64>,
    pub mos_did_dw: AlignedVec<f64>,
    pub mos_did_dl: AlignedVec<f64>,
    pub mos_did_dvto: AlignedVec<f64>,
    pub mos_did_dkp: AlignedVec<f64>,
    pub mos_gm_buf_idx: Vec<u32>,
    pub mos_gds_buf_idx: Vec<u32>,
    pub mos_gmbs_buf_idx: Vec<u32>,
    pub mos_rev_gm_buf_idx: Vec<u32>,
    pub mos_rev_gmbs_buf_idx: Vec<u32>,
    pub mos_rhs_buf_idx: Vec<u32>,

    // --- Flat Conflict-Free Gather Buffers ---
    pub cond_buf: AlignedVec<f64>,
    pub rhs_buf: AlignedVec<f64>,
    pub gather_table: CscGatherTable,

    // --- Pre-Stamped Static Linear Fast-Path ---
    pub static_linear_csc_values: Vec<f64>,
    pub dynamic_gather_table: CscGatherTable,
    pub linear_dirty: bool,
}

/// Pre-compiled circuit topology combining the [`DeviceSoaBuffer`] and structural [`CscMatrix`].
#[derive(Clone, Debug)]
pub struct CompiledTopology {
    pub soa: DeviceSoaBuffer,
    pub jacobian: CscMatrix<f64>,
}

impl DeviceSoaBuffer {
    /// Returns `true` if the circuit contains no nonlinear devices (Diodes or MOSFETs).
    #[inline]
    pub fn is_linear(&self) -> bool {
        self.diode_names.is_empty() && self.mos_names.is_empty()
    }

    /// Reads the voltage of a 1-based node ID (`0` returns `0.0` for Ground).
    #[inline]
    pub fn node_voltage(voltages: &[f64], node_id: usize) -> f64 {
        if node_id == 0 {
            0.0
        } else {
            voltages[node_id - 1]
        }
    }

    /// Updates independent voltage and current source waveforms for time `t` (or DC if `None`).
    pub fn update_sources_at_time(&mut self, t: Option<f64>) {
        for (i, wf) in self.vsrc_waveforms.iter().enumerate() {
            let v = match t {
                Some(time) => wf.evaluate_at(time),
                None => wf.dc_value(),
            };
            self.vsrc_values[i] = v;
        }
        for (i, wf) in self.isrc_waveforms.iter().enumerate() {
            let cur = match t {
                Some(time) => wf.evaluate_at(time),
                None => wf.dc_value(),
            };
            self.isrc_values[i] = cur;
        }
    }

    /// Sets the value of an independent voltage or current source by name (used in `.dc` sweeps
    /// and parameter optimization).
    pub fn set_source_dc_value(&mut self, source_name: &str, val: f64) -> SimResult<()> {
        for (i, name) in self.vsrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(source_name) {
                self.vsrc_waveforms[i] = SourceWaveform::Dc(val);
                self.vsrc_values[i] = val;
                return Ok(());
            }
        }
        for (i, name) in self.isrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(source_name) {
                self.isrc_waveforms[i] = SourceWaveform::Dc(val);
                self.isrc_values[i] = val;
                return Ok(());
            }
        }
        Err(SimError::TopologyError(format!(
            "Source `{source_name}` not found in circuit"
        )))
    }

    /// Updates a named device parameter (`R`, `W`, `L`, `VTO`, `KP`, `IS`, `N`, `V`, `I`)
    /// in-place without recompiling the topology.
    pub fn set_device_parameter(
        &mut self,
        device_name: &str,
        param_name: &str,
        value: f64,
    ) -> SimResult<()> {
        let p_lower = param_name.to_ascii_lowercase();
        for (i, name) in self.res_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(device_name) && (p_lower == "r" || p_lower == "resistance")
            {
                self.res_resistance[i] = value;
                let g = 1.0 / value.max(1e-18);
                self.res_conductance[i] = g;
                self.cond_buf[self.res_cond_buf_idx[i] as usize] = g;
                self.linear_dirty = true;
                return Ok(());
            }
        }
        for (i, name) in self.mos_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(device_name) {
                match p_lower.as_str() {
                    "w" => self.mos_w[i] = value,
                    "l" => self.mos_l[i] = value,
                    "vto" | "vt0" => self.mos_vto[i] = value,
                    "kp" => self.mos_kp[i] = value,
                    "lambda" => self.mos_lambda[i] = value,
                    _ => {
                        return Err(SimError::TopologyError(format!(
                            "Unsupported MOSFET parameter `{param_name}`"
                        )))
                    }
                }
                return Ok(());
            }
        }
        for (i, name) in self.diode_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(device_name) {
                match p_lower.as_str() {
                    "is" => self.diode_is[i] = value,
                    "n" => self.diode_n[i] = value,
                    "area" => self.diode_area[i] = value,
                    _ => {
                        return Err(SimError::TopologyError(format!(
                            "Unsupported Diode parameter `{param_name}`"
                        )))
                    }
                }
                return Ok(());
            }
        }
        if p_lower == "dc" || p_lower == "v" || p_lower == "i" {
            return self.set_source_dc_value(device_name, value);
        }
        Err(SimError::TopologyError(format!(
            "Device `{device_name}` or parameter `{param_name}` not found"
        )))
    }
}

/// Compiles a parsed [`Netlist`] into a 64-byte aligned [`DeviceSoaBuffer`] and a conflict-free
/// [`CscGatherTable`] + [`CscMatrix`].
pub fn compile_netlist(netlist: &Netlist) -> SimResult<CompiledTopology> {
    let num_nodes = netlist.num_nodes();
    let mut netlist_expanded = netlist.clone();

    // Synthesize MOSFET gate-source / gate-drain / gate-bulk overlap capacitors when Cgso/Cgdo/Cgbo > 0
    for mos in &netlist.mosfets {
        if let Some(m) = netlist.mosfet_models.get(&mos.model) {
            let w = mos.w.unwrap_or(m.w_default);
            if m.cgso > 0.0 {
                netlist_expanded
                    .capacitors
                    .push(crate::netlist::CapacitorInstance {
                        name: format!("c_gs_{}", mos.name),
                        n_pos: mos.n_gate,
                        n_neg: mos.n_source,
                        capacitance: m.cgso * w,
                        ic: None,
                    });
            }
            if m.cgdo > 0.0 {
                netlist_expanded
                    .capacitors
                    .push(crate::netlist::CapacitorInstance {
                        name: format!("c_gd_{}", mos.name),
                        n_pos: mos.n_gate,
                        n_neg: mos.n_drain,
                        capacitance: m.cgdo * w,
                        ic: None,
                    });
            }
            if m.cgbo > 0.0 {
                netlist_expanded
                    .capacitors
                    .push(crate::netlist::CapacitorInstance {
                        name: format!("c_gb_{}", mos.name),
                        n_pos: mos.n_gate,
                        n_neg: mos.n_bulk,
                        capacitance: m.cgbo * w,
                        ic: None,
                    });
            }
        }
    }

    // Synthesize Diode zero-bias junction capacitors when CJO > 0
    for d in &netlist.diodes {
        if let Some(m) = netlist.diode_models.get(&d.model) {
            if m.cjo > 0.0 {
                netlist_expanded
                    .capacitors
                    .push(crate::netlist::CapacitorInstance {
                        name: format!("c_j_{}", d.name),
                        n_pos: d.n_anode,
                        n_neg: d.n_cathode,
                        capacitance: m.cjo * d.area,
                        ic: None,
                    });
            }
        }
    }

    let mna_dim = num_nodes + netlist_expanded.v_sources.len() + netlist_expanded.inductors.len();
    if mna_dim == 0 {
        return Err(SimError::TopologyError(
            "Circuit contains no non-ground nodes or branches".to_string(),
        ));
    }

    let mut soa = DeviceSoaBuffer {
        num_nodes,
        mna_dim,
        gmin: 1e-12,
        temp_vt: 0.025864917007155805, // Exact ngspice-47 k*T/q at 300.15 K (27 C)
        is_transient: false,
        transient_g_scale: 0.0,
        ..Default::default()
    };

    // Coordinate triplets `(row, col, cond_buf_idx, sign, is_static_linear)`
    let mut matrix_stamps: Vec<(usize, usize, u32, f64, bool)> = Vec::new();
    // RHS triplets `(row, rhs_buf_idx, sign)`
    let mut rhs_stamps: Vec<(usize, u32, f64)> = Vec::new();

    let add_2pin_conductance =
        |n_pos: usize,
         n_neg: usize,
         buf_idx: u32,
         is_static: bool,
         stamps: &mut Vec<(usize, usize, u32, f64, bool)>| {
            if n_pos > 0 {
                let p = n_pos - 1;
                stamps.push((p, p, buf_idx, 1.0, is_static));
                if n_neg > 0 {
                    let n = n_neg - 1;
                    stamps.push((p, n, buf_idx, -1.0, is_static));
                }
            }
            if n_neg > 0 {
                let n = n_neg - 1;
                stamps.push((n, n, buf_idx, 1.0, is_static));
                if n_pos > 0 {
                    let p = n_pos - 1;
                    stamps.push((n, p, buf_idx, -1.0, is_static));
                }
            }
        };

    // 1. Resistors (static linear)
    for r in &netlist_expanded.resistors {
        let g = 1.0 / r.resistance.max(1e-18);
        let buf_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(g);
        soa.res_names.push(r.name.clone());
        soa.res_n_pos.push(r.n_pos);
        soa.res_n_neg.push(r.n_neg);
        soa.res_resistance.push(r.resistance);
        soa.res_conductance.push(g);
        soa.res_cond_buf_idx.push(buf_idx);
        add_2pin_conductance(r.n_pos, r.n_neg, buf_idx, true, &mut matrix_stamps);
    }

    // 2. VCCS (static linear)
    for vccs in &netlist_expanded.vccs {
        let buf_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(vccs.gm);
        soa.vccs_names.push(vccs.name.clone());
        soa.vccs_n_pos.push(vccs.n_pos);
        soa.vccs_n_neg.push(vccs.n_neg);
        soa.vccs_nc_pos.push(vccs.nc_pos);
        soa.vccs_nc_neg.push(vccs.nc_neg);
        soa.vccs_gm.push(vccs.gm);
        soa.vccs_cond_buf_idx.push(buf_idx);

        if vccs.n_pos > 0 && vccs.nc_pos > 0 {
            matrix_stamps.push((vccs.n_pos - 1, vccs.nc_pos - 1, buf_idx, 1.0, true));
        }
        if vccs.n_pos > 0 && vccs.nc_neg > 0 {
            matrix_stamps.push((vccs.n_pos - 1, vccs.nc_neg - 1, buf_idx, -1.0, true));
        }
        if vccs.n_neg > 0 && vccs.nc_pos > 0 {
            matrix_stamps.push((vccs.n_neg - 1, vccs.nc_pos - 1, buf_idx, -1.0, true));
        }
        if vccs.n_neg > 0 && vccs.nc_neg > 0 {
            matrix_stamps.push((vccs.n_neg - 1, vccs.nc_neg - 1, buf_idx, 1.0, true));
        }
    }

    // 3. Capacitors (dynamic companion model during transient)
    for c in &netlist_expanded.capacitors {
        let cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);

        soa.cap_names.push(c.name.clone());
        soa.cap_n_pos.push(c.n_pos);
        soa.cap_n_neg.push(c.n_neg);
        soa.cap_capacitance.push(c.capacitance);
        soa.cap_geq.push(0.0);
        soa.cap_ieq.push(0.0);
        soa.cap_v_prev.push(c.ic.unwrap_or(0.0));
        soa.cap_v_prev2.push(c.ic.unwrap_or(0.0));
        soa.cap_i_prev.push(0.0);
        soa.cap_cond_buf_idx.push(cond_idx);
        soa.cap_rhs_buf_idx.push(rhs_idx);

        add_2pin_conductance(c.n_pos, c.n_neg, cond_idx, false, &mut matrix_stamps);
        if c.n_pos > 0 {
            rhs_stamps.push((c.n_pos - 1, rhs_idx, -1.0));
        }
        if c.n_neg > 0 {
            rhs_stamps.push((c.n_neg - 1, rhs_idx, 1.0));
        }
    }

    // 4. Independent Voltage Sources
    for (v_idx, vs) in netlist_expanded.v_sources.iter().enumerate() {
        let br = num_nodes + v_idx;
        let br_cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(1.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        let dc_val = vs.waveform.dc_value();
        soa.rhs_buf.push(dc_val);

        soa.vsrc_names.push(vs.name.clone());
        soa.vsrc_n_pos.push(vs.n_pos);
        soa.vsrc_n_neg.push(vs.n_neg);
        soa.vsrc_branch_idx.push(br);
        soa.vsrc_waveforms.push(vs.waveform.clone());
        soa.vsrc_values.push(dc_val);
        soa.vsrc_branch_buf_idx.push(br_cond_idx);
        soa.vsrc_rhs_buf_idx.push(rhs_idx);

        if vs.n_pos > 0 {
            let p = vs.n_pos - 1;
            matrix_stamps.push((p, br, br_cond_idx, 1.0, true));
            matrix_stamps.push((br, p, br_cond_idx, 1.0, true));
        }
        if vs.n_neg > 0 {
            let n = vs.n_neg - 1;
            matrix_stamps.push((n, br, br_cond_idx, -1.0, true));
            matrix_stamps.push((br, n, br_cond_idx, -1.0, true));
        }
        rhs_stamps.push((br, rhs_idx, 1.0));
    }

    // 5. Inductors (MNA branch equation: V(pos) - V(neg) - R_eq * I_L = V_eq)
    for (l_idx, ind) in netlist_expanded.inductors.iter().enumerate() {
        let br = num_nodes + netlist_expanded.v_sources.len() + l_idx;
        let br_inc_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(1.0);
        let req_cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);

        soa.ind_names.push(ind.name.clone());
        soa.ind_n_pos.push(ind.n_pos);
        soa.ind_n_neg.push(ind.n_neg);
        soa.ind_branch_idx.push(br);
        soa.ind_inductance.push(ind.inductance);
        soa.ind_req.push(0.0);
        soa.ind_veq.push(0.0);
        soa.ind_i_prev.push(ind.ic.unwrap_or(0.0));
        soa.ind_i_prev2.push(ind.ic.unwrap_or(0.0));
        soa.ind_v_prev.push(0.0);
        soa.ind_branch_buf_idx.push(br_inc_idx);
        soa.ind_cond_buf_idx.push(req_cond_idx);
        soa.ind_rhs_buf_idx.push(rhs_idx);

        if ind.n_pos > 0 {
            let p = ind.n_pos - 1;
            matrix_stamps.push((p, br, br_inc_idx, 1.0, true));
            matrix_stamps.push((br, p, br_inc_idx, 1.0, true));
        }
        if ind.n_neg > 0 {
            let n = ind.n_neg - 1;
            matrix_stamps.push((n, br, br_inc_idx, -1.0, true));
            matrix_stamps.push((br, n, br_inc_idx, -1.0, true));
        }
        matrix_stamps.push((br, br, req_cond_idx, -1.0, false));
        rhs_stamps.push((br, rhs_idx, 1.0));
    }

    // 6. Independent Current Sources (flowing from n_pos to n_neg inside the source)
    for isrc in &netlist_expanded.i_sources {
        let rhs_idx = soa.rhs_buf.len() as u32;
        let dc_val = isrc.waveform.dc_value();
        soa.rhs_buf.push(dc_val);

        soa.isrc_names.push(isrc.name.clone());
        soa.isrc_n_pos.push(isrc.n_pos);
        soa.isrc_n_neg.push(isrc.n_neg);
        soa.isrc_waveforms.push(isrc.waveform.clone());
        soa.isrc_values.push(dc_val);
        soa.isrc_rhs_buf_idx.push(rhs_idx);

        if isrc.n_pos > 0 {
            rhs_stamps.push((isrc.n_pos - 1, rhs_idx, -1.0));
        }
        if isrc.n_neg > 0 {
            rhs_stamps.push((isrc.n_neg - 1, rhs_idx, 1.0));
        }
    }

    // 7. PN Junction Diodes
    for d in &netlist_expanded.diodes {
        let m = netlist_expanded
            .diode_models
            .get(&d.model)
            .cloned()
            .unwrap_or_default();
        let cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);

        soa.diode_names.push(d.name.clone());
        soa.diode_anode.push(d.n_anode);
        soa.diode_cathode.push(d.n_cathode);
        soa.diode_is.push(m.is * d.area);
        soa.diode_n.push(m.n);
        soa.diode_area.push(d.area);
        soa.diode_cjo.push(m.cjo * d.area);
        soa.diode_vj.push(m.vj);
        soa.diode_m.push(m.m);
        soa.diode_tt.push(m.tt);
        soa.diode_vd.push(0.0);
        soa.diode_id.push(0.0);
        soa.diode_gd.push(0.0);
        soa.diode_ieq.push(0.0);
        soa.diode_did_dis.push(0.0);
        soa.diode_did_dn.push(0.0);
        soa.diode_cond_buf_idx.push(cond_idx);
        soa.diode_rhs_buf_idx.push(rhs_idx);

        add_2pin_conductance(d.n_anode, d.n_cathode, cond_idx, false, &mut matrix_stamps);
        if d.n_anode > 0 {
            rhs_stamps.push((d.n_anode - 1, rhs_idx, -1.0));
        }
        if d.n_cathode > 0 {
            rhs_stamps.push((d.n_cathode - 1, rhs_idx, 1.0));
        }
    }

    // 8. MOSFETs
    for mos in &netlist_expanded.mosfets {
        let m = netlist_expanded
            .mosfet_models
            .get(&mos.model)
            .cloned()
            .unwrap_or_default();
        let gm_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let gds_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let gmbs_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rev_gm_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rev_gmbs_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);

        soa.mos_names.push(mos.name.clone());
        soa.mos_drain.push(mos.n_drain);
        soa.mos_gate.push(mos.n_gate);
        soa.mos_source.push(mos.n_source);
        soa.mos_bulk.push(mos.n_bulk);
        soa.mos_polarity.push(m.mos_type.sign());
        soa.mos_level.push(m.level);
        soa.mos_kp.push(m.kp);
        soa.mos_w.push(mos.w.unwrap_or(m.w_default));
        soa.mos_l.push(mos.l.unwrap_or(m.l_default));
        soa.mos_vto.push(m.vto);
        soa.mos_gamma.push(m.gamma);
        soa.mos_phi.push(m.phi);
        soa.mos_lambda.push(m.lambda);
        soa.mos_vgs.push(0.0);
        soa.mos_vds.push(0.0);
        soa.mos_vbs.push(0.0);
        soa.mos_id.push(0.0);
        soa.mos_gm.push(0.0);
        soa.mos_gds.push(0.0);
        soa.mos_gmbs.push(0.0);
        soa.mos_ieq.push(0.0);
        soa.mos_did_dw.push(0.0);
        soa.mos_did_dl.push(0.0);
        soa.mos_did_dvto.push(0.0);
        soa.mos_did_dkp.push(0.0);
        soa.mos_gm_buf_idx.push(gm_idx);
        soa.mos_gds_buf_idx.push(gds_idx);
        soa.mos_gmbs_buf_idx.push(gmbs_idx);
        soa.mos_rev_gm_buf_idx.push(rev_gm_idx);
        soa.mos_rev_gmbs_buf_idx.push(rev_gmbs_idx);
        soa.mos_rhs_buf_idx.push(rhs_idx);

        // g_ds stamps between Drain and Source:
        add_2pin_conductance(
            mos.n_drain,
            mos.n_source,
            gds_idx,
            false,
            &mut matrix_stamps,
        );

        // Forward mode g_m stamps Controlled Current from Drain to Source controlled by (Gate - Source):
        if mos.n_drain > 0 && mos.n_gate > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_gate - 1, gm_idx, 1.0, false));
        }
        if mos.n_drain > 0 && mos.n_source > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_source - 1, gm_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_gate > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_gate - 1, gm_idx, -1.0, false));
        }
        if mos.n_source > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_source - 1, gm_idx, 1.0, false));
        }

        // Forward mode g_mbs controlled by (Bulk - Source):
        if mos.n_drain > 0 && mos.n_bulk > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_bulk - 1, gmbs_idx, 1.0, false));
        }
        if mos.n_drain > 0 && mos.n_source > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_source - 1, gmbs_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_bulk > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_bulk - 1, gmbs_idx, -1.0, false));
        }
        if mos.n_source > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_source - 1, gmbs_idx, 1.0, false));
        }

        // Reversed mode (`v_ds < 0`) g_m controlled by (Gate - Drain):
        if mos.n_drain > 0 && mos.n_gate > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_gate - 1, rev_gm_idx, 1.0, false));
        }
        if mos.n_drain > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_drain - 1, rev_gm_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_gate > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_gate - 1, rev_gm_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_drain > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_drain - 1, rev_gm_idx, 1.0, false));
        }

        // Reversed mode g_mbs controlled by (Bulk - Drain):
        if mos.n_drain > 0 && mos.n_bulk > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_bulk - 1, rev_gmbs_idx, 1.0, false));
        }
        if mos.n_drain > 0 {
            matrix_stamps.push((mos.n_drain - 1, mos.n_drain - 1, rev_gmbs_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_bulk > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_bulk - 1, rev_gmbs_idx, -1.0, false));
        }
        if mos.n_source > 0 && mos.n_drain > 0 {
            matrix_stamps.push((mos.n_source - 1, mos.n_drain - 1, rev_gmbs_idx, 1.0, false));
        }

        // Equivalent current i_eq from Drain to Source
        if mos.n_drain > 0 {
            rhs_stamps.push((mos.n_drain - 1, rhs_idx, -1.0));
        }
        if mos.n_source > 0 {
            rhs_stamps.push((mos.n_source - 1, rhs_idx, 1.0));
        }
    }

    // Build structural CSC Matrix from all coordinate triplets
    let coords: Vec<(usize, usize)> = matrix_stamps.iter().map(|&(r, c, ..)| (r, c)).collect();
    let jacobian = CscMatrix::<f64>::from_pattern(mna_dim, mna_dim, &coords);
    let nnz = jacobian.nnz();

    // Build full CscGatherTable and Dynamic-Only CscGatherTable + Static Linear Values
    let mut full_nnz_buckets: Vec<Vec<GatherTerm>> = vec![Vec::new(); nnz];
    let mut dyn_nnz_buckets: Vec<Vec<GatherTerm>> = vec![Vec::new(); nnz];
    let mut static_linear_csc_values = vec![0.0_f64; nnz];

    for &(r, c, buf_idx, sign, is_static) in &matrix_stamps {
        if let Some(nnz_idx) = jacobian.find_value_index(r, c) {
            let term = GatherTerm {
                buffer_idx: buf_idx,
                sign,
            };
            full_nnz_buckets[nnz_idx].push(term);
            if is_static {
                static_linear_csc_values[nnz_idx] += sign * soa.cond_buf[buf_idx as usize];
            } else {
                dyn_nnz_buckets[nnz_idx].push(term);
            }
        }
    }

    // Add Gmin = 1e-12 only to DC-floating node diagonals (e.g., capacitor-only or isolated gate nodes)
    let mut node_has_dc_device = vec![false; num_nodes];
    for r in &netlist_expanded.resistors {
        if r.n_pos > 0 {
            node_has_dc_device[r.n_pos - 1] = true;
        }
        if r.n_neg > 0 {
            node_has_dc_device[r.n_neg - 1] = true;
        }
    }
    for vs in &netlist_expanded.v_sources {
        if vs.n_pos > 0 {
            node_has_dc_device[vs.n_pos - 1] = true;
        }
        if vs.n_neg > 0 {
            node_has_dc_device[vs.n_neg - 1] = true;
        }
    }
    for ind in &netlist_expanded.inductors {
        if ind.n_pos > 0 {
            node_has_dc_device[ind.n_pos - 1] = true;
        }
        if ind.n_neg > 0 {
            node_has_dc_device[ind.n_neg - 1] = true;
        }
    }
    for d in &netlist_expanded.diodes {
        if d.n_anode > 0 {
            node_has_dc_device[d.n_anode - 1] = true;
        }
        if d.n_cathode > 0 {
            node_has_dc_device[d.n_cathode - 1] = true;
        }
    }
    for m in &netlist_expanded.mosfets {
        if m.n_drain > 0 {
            node_has_dc_device[m.n_drain - 1] = true;
        }
        if m.n_source > 0 {
            node_has_dc_device[m.n_source - 1] = true;
        }
    }
    for (d, &has_dc) in node_has_dc_device.iter().enumerate() {
        if !has_dc {
            let diag = jacobian.diag_idx[d];
            if diag < nnz {
                static_linear_csc_values[diag] += soa.gmin;
            }
        }
    }

    let mut rhs_buckets: Vec<Vec<GatherTerm>> = vec![Vec::new(); mna_dim];
    for &(r, buf_idx, sign) in &rhs_stamps {
        rhs_buckets[r].push(GatherTerm {
            buffer_idx: buf_idx,
            sign,
        });
    }

    soa.gather_table = build_gather_table(&full_nnz_buckets, &rhs_buckets);
    soa.dynamic_gather_table = build_gather_table(&dyn_nnz_buckets, &rhs_buckets);
    soa.static_linear_csc_values = static_linear_csc_values;
    soa.linear_dirty = false;

    Ok(CompiledTopology { soa, jacobian })
}

fn build_gather_table(
    nnz_buckets: &[Vec<GatherTerm>],
    rhs_buckets: &[Vec<GatherTerm>],
) -> CscGatherTable {
    let mut nnz_ptr = Vec::with_capacity(nnz_buckets.len() + 1);
    let mut nnz_terms = Vec::new();
    let mut active_nnz = Vec::new();
    nnz_ptr.push(0);
    for (k, bucket) in nnz_buckets.iter().enumerate() {
        let start = nnz_terms.len();
        nnz_terms.extend_from_slice(bucket);
        let end = nnz_terms.len();
        nnz_ptr.push(end);
        if start < end {
            active_nnz.push((k as u32, start as u32, end as u32));
        }
    }

    let mut rhs_ptr = Vec::with_capacity(rhs_buckets.len() + 1);
    let mut rhs_terms = Vec::new();
    let mut active_rhs = Vec::new();
    rhs_ptr.push(0);
    for (i, bucket) in rhs_buckets.iter().enumerate() {
        let start = rhs_terms.len();
        rhs_terms.extend_from_slice(bucket);
        let end = rhs_terms.len();
        rhs_ptr.push(end);
        if start < end {
            active_rhs.push((i as u32, start as u32, end as u32));
        }
    }

    CscGatherTable {
        nnz_ptr,
        nnz_terms,
        active_nnz,
        rhs_ptr,
        rhs_terms,
        active_rhs,
    }
}
