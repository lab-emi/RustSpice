//! 64-byte cache-line aligned Structure-of-Arrays (`DeviceSoaBuffer`) and pre-compiled
//! conflict-free sparse matrix topology (`CompiledTopology`).

use std::ops::{Deref, DerefMut};

use rustspice_macros::spice_device;
use serde::{Deserialize, Serialize};

use crate::error::{SimError, SimResult};
use crate::ext::{ExtDevices, Junction, MeyerCaps, Mutual, PolySource};
use crate::netlist::{Netlist, ResistorInstance, SourceWaveform};
use crate::sparse::{CscGatherTable, CscMatrix, GatherTerm, JaggedTerms};

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
    /// Junction `gmin` of the deck (`.options gmin`, default 1e-12 S); `gmin` rises above it only during gmin
    /// stepping, and the excess is stamped as a shunt on every node.
    pub gmin_nominal: f64,
    pub temp_vt: f64,
    pub is_transient: bool,
    pub transient_g_scale: f64,
    /// Set during `evaluate_devices` when any junction limiter (`pnjlim` / exponent clamp)
    /// modified a trial terminal voltage so the outer Newton loop does not accept a damped step as converged.
    pub limiter_active: bool,
    /// Circuit node names by 1-based node id (`node_names[0]` is ground); used for diagnostics only.
    pub node_names: Vec<String>,

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
    /// Element `ic=` voltage (NaN when absent); applied only by a `.tran ... uic` start.
    pub cap_ic: Vec<f64>,

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
    /// Element `ic=` current (NaN when absent); applied only by a `.tran ... uic` start.
    pub ind_ic: Vec<f64>,

    // --- `.ic` node clamps (Norton equivalents, active only while the transient start point is solved) ---
    pub ic_nodes: Vec<usize>,
    pub ic_values: Vec<f64>,
    pub ic_cond_buf_idx: Vec<u32>,
    pub ic_rhs_buf_idx: Vec<u32>,
    // --- `.nodeset` node clamps (same form, active only during the first phase of an operating point) ---
    pub nodeset_nodes: Vec<usize>,
    pub nodeset_values: Vec<f64>,
    pub nodeset_cond_buf_idx: Vec<u32>,
    pub nodeset_rhs_buf_idx: Vec<u32>,

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

    // --- Controlled sources: `E` / `H` branch rows (after the inductor branches) and current outputs ---
    pub ctl_names: Vec<String>,
    pub ctl_n_pos: Vec<usize>,
    pub ctl_n_neg: Vec<usize>,
    pub ctl_branch_idx: Vec<usize>,
    /// Output nodes `(n+, n-)` of the `F` and `G ... POLY` sources (current outputs, like a VCCS).
    pub ctl_current_nodes: Vec<(usize, usize)>,

    // --- Couplings, polynomial sources, full junctions and Meyer capacitances (see `ext`) ---
    pub ext: ExtDevices,

    // --- Flat Conflict-Free Gather Buffers ---
    pub cond_buf: AlignedVec<f64>,
    pub rhs_buf: AlignedVec<f64>,
    pub gather_table: CscGatherTable,

    // --- Pre-Stamped Static Linear Fast-Path ---
    pub static_linear_csc_values: Vec<f64>,
    pub dynamic_gather_table: CscGatherTable,
    pub dc_floating_diag_idx: Vec<usize>,
    pub linear_dirty: bool,
}

/// Pre-compiled circuit topology combining the [`DeviceSoaBuffer`] and structural [`CscMatrix`].
#[derive(Clone, Debug)]
pub struct CompiledTopology {
    pub soa: DeviceSoaBuffer,
    pub jacobian: CscMatrix<f64>,
}

impl DeviceSoaBuffer {
    /// Minimum resistance magnitude substituted when a netlist specifies `R = 0` (`1 uOhm`).
    /// Negative resistances (`R < 0`) are preserved with their exact signed conductance `1 / R`.
    pub const ZERO_OHM_SUBSTITUTE: f64 = ZERO_OHM_SUBSTITUTE;

    #[inline]
    pub fn resistor_conductance(_name: &str, r: f64) -> f64 {
        resistor_conductance(r)
    }

    /// Returns `true` if the circuit contains no nonlinear devices (diodes, MOSFETs, junctions, polynomial
    /// sources).
    #[inline]
    pub fn is_linear(&self) -> bool {
        self.diode_names.is_empty() && self.mos_names.is_empty() && !self.ext.is_nonlinear()
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
            self.rhs_buf[self.vsrc_rhs_buf_idx[i] as usize] = v;
        }
        for (i, wf) in self.isrc_waveforms.iter().enumerate() {
            let cur = match t {
                Some(time) => wf.evaluate_at(time),
                None => wf.dc_value(),
            };
            self.isrc_values[i] = cur;
            self.rhs_buf[self.isrc_rhs_buf_idx[i] as usize] = cur;
        }
    }

    /// Sets the value of an independent voltage or current source by name (used in `.dc` sweeps
    /// and parameter optimization).
    pub fn set_source_dc_value(&mut self, source_name: &str, val: f64) -> SimResult<()> {
        for (i, name) in self.vsrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(source_name) {
                self.vsrc_waveforms[i] = SourceWaveform::Dc(val);
                self.vsrc_values[i] = val;
                self.rhs_buf[self.vsrc_rhs_buf_idx[i] as usize] = val;
                return Ok(());
            }
        }
        for (i, name) in self.isrc_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(source_name) {
                self.isrc_waveforms[i] = SourceWaveform::Dc(val);
                self.isrc_values[i] = val;
                self.rhs_buf[self.isrc_rhs_buf_idx[i] as usize] = val;
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
                let g = Self::resistor_conductance(device_name, value);
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
                    "area" => {
                        let old_area = self.diode_area[i];
                        if old_area > 0.0 {
                            let scale = value / old_area;
                            self.diode_is[i] *= scale;
                            self.diode_cjo[i] *= scale;
                        }
                        self.diode_area[i] = value;
                    }
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

    /// Conductance of the `.ic` node clamps while they are active (a 1 nano-ohm Norton source: the clamped node
    /// sits within `I / 1e9` volts of its value for any circuit current `I`).
    pub const IC_CLAMP_CONDUCTANCE: f64 = 1e9;

    /// Activates (`true`) or removes (`false`) the `.nodeset` node clamps.
    pub fn set_nodeset_clamps(&mut self, active: bool) {
        let g = if active {
            Self::IC_CLAMP_CONDUCTANCE
        } else {
            0.0
        };
        for i in 0..self.nodeset_nodes.len() {
            self.cond_buf[self.nodeset_cond_buf_idx[i] as usize] = g;
            self.rhs_buf[self.nodeset_rhs_buf_idx[i] as usize] = g * self.nodeset_values[i];
        }
    }

    /// Activates (`true`) or removes (`false`) the `.ic` node clamps.
    pub fn set_ic_clamps(&mut self, active: bool) {
        let g = if active {
            Self::IC_CLAMP_CONDUCTANCE
        } else {
            0.0
        };
        for i in 0..self.ic_nodes.len() {
            self.cond_buf[self.ic_cond_buf_idx[i] as usize] = g;
            self.rhs_buf[self.ic_rhs_buf_idx[i] as usize] = g * self.ic_values[i];
        }
    }

    /// Detects KCL-inconsistent DC cutsets: any connected component of DC-conductive branches
    /// (`R`, `V`, `L`, `D`, MOSFET drain-source channel, VCCS output) not containing ground (`0`)
    /// into which independent current sources inject a non-zero net DC current.
    ///
    /// Returns the 1-based node ID of a representative node in the first offending cutset,
    /// or `None` if all floating DC cutsets have zero net current injection.
    pub fn find_dc_current_cutset(
        &self,
        fixed_nodes: Option<&rustc_hash::FxHashMap<usize, f64>>,
    ) -> Option<usize> {
        let total = self.num_nodes + 1;
        let mut parent: Vec<usize> = (0..total).collect();

        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                let p = parent[x];
                parent[x] = parent[p];
                x = p;
            }
            x
        }
        fn union(parent: &mut [usize], a: usize, b: usize) {
            let ra = find(parent, a);
            let rb = find(parent, b);
            if ra != rb {
                if ra < rb {
                    parent[rb] = ra;
                } else {
                    parent[ra] = rb;
                }
            }
        }

        if let Some(ic_map) = fixed_nodes {
            for &node_id in ic_map.keys() {
                if node_id < total {
                    union(&mut parent, 0, node_id);
                }
            }
        }

        for i in 0..self.res_names.len() {
            union(&mut parent, self.res_n_pos[i], self.res_n_neg[i]);
        }
        for i in 0..self.vsrc_names.len() {
            union(&mut parent, self.vsrc_n_pos[i], self.vsrc_n_neg[i]);
        }
        for i in 0..self.ind_names.len() {
            union(&mut parent, self.ind_n_pos[i], self.ind_n_neg[i]);
        }
        for i in 0..self.diode_names.len() {
            union(&mut parent, self.diode_anode[i], self.diode_cathode[i]);
        }
        for i in 0..self.mos_names.len() {
            union(&mut parent, self.mos_drain[i], self.mos_source[i]);
        }
        for i in 0..self.vccs_names.len() {
            if self.vccs_gm[i].abs() > 0.0 {
                union(&mut parent, self.vccs_n_pos[i], self.vccs_n_neg[i]);
            }
        }
        for i in 0..self.ctl_names.len() {
            union(&mut parent, self.ctl_n_pos[i], self.ctl_n_neg[i]);
        }
        for &(p, q) in &self.ctl_current_nodes {
            union(&mut parent, p, q);
        }
        for (a, c) in self.ext.junction_terminals() {
            union(&mut parent, a, c);
        }

        let mut net_isrc = vec![0.0_f64; total];
        let mut scale_isrc = vec![0.0_f64; total];
        for i in 0..self.isrc_names.len() {
            let cur = self.isrc_values[i];
            if cur == 0.0 {
                continue;
            }
            let rp = find(&mut parent, self.isrc_n_pos[i]);
            let rn = find(&mut parent, self.isrc_n_neg[i]);
            net_isrc[rp] -= cur;
            net_isrc[rn] += cur;
            scale_isrc[rp] += cur.abs();
            scale_isrc[rn] += cur.abs();
        }

        let gnd_root = find(&mut parent, 0);
        (1..total).find(|&r| {
            find(&mut parent, r) == r
                && r != gnd_root
                && scale_isrc[r] > 0.0
                && net_isrc[r].abs() > 1e-12 * scale_isrc[r]
        })
    }

    /// Verifies that the circuit has a well-posed DC operating point, returning
    /// [`SimError::SingularMatrix`] if a floating DC cutset has a non-zero net current injection.
    pub fn check_dc_solvable_with_ic(
        &self,
        fixed_nodes: Option<&rustc_hash::FxHashMap<usize, f64>>,
    ) -> SimResult<()> {
        if let Some((nodes, _)) = find_dc_current_cutset(self) {
            if let Some(ic_map) = fixed_nodes {
                if nodes.iter().all(|k| ic_map.contains_key(k)) {
                    return Ok(());
                }
            }
            return Err(SimError::SingularMatrix {
                index: nodes[0] - 1,
            });
        }
        Ok(())
    }
}

/// Resistance stamped for a resistor written with value `R = 0` (an ideal short, whose conductance is
/// infinite). 1 nano-ohm keeps the stamped conductance within 1e9 S, so that the LU factorisation of
/// a zero-ohm link in series with a 1 kohm load is exact to ~1e-4 while a short to ground stays below 1e-11 V.
/// Reported by the CLI as a warning.
pub const ZERO_OHM_SUBSTITUTE: f64 = 1e-9;

/// `true` if a resistor of value `r` is not stamped as written (only an exact zero is).
#[inline]
pub fn resistance_is_floored(r: f64) -> bool {
    r == 0.0
}

/// Conductance stamped for a resistor of value `r` ohm. Negative resistances are legal (negative
/// conductance, exactly as written); positive ones keep the historical 1e-18 ohm lower bound.
#[inline]
pub fn resistor_conductance(r: f64) -> f64 {
    if r == 0.0 {
        1.0 / ZERO_OHM_SUBSTITUTE
    } else if r < 0.0 {
        1.0 / r
    } else {
        1.0 / r.max(1e-18)
    }
}

/// Detects a DC problem that has NO solution because of KCL: a connected island of elements that
/// carry DC current (R, L, V, diode, MOS channel) and does not contain ground, yet receives a net
/// non-zero current from independent current sources (e.g. a node touched only by a current
/// source and capacitors). `gmin` would regularise such a circuit into `I / gmin` volts.
/// Returns `(island nodes, net injected current)`; islands touched by a VCCS output are skipped.
pub fn find_dc_current_cutset(soa: &DeviceSoaBuffer) -> Option<(Vec<usize>, f64)> {
    let n = soa.num_nodes + 1;
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    let union = |a: usize, b: usize, parent: &mut Vec<usize>| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    };
    for i in 0..soa.res_names.len() {
        union(soa.res_n_pos[i], soa.res_n_neg[i], &mut parent);
    }
    for i in 0..soa.ind_names.len() {
        union(soa.ind_n_pos[i], soa.ind_n_neg[i], &mut parent);
    }
    for i in 0..soa.vsrc_names.len() {
        union(soa.vsrc_n_pos[i], soa.vsrc_n_neg[i], &mut parent);
    }
    for i in 0..soa.diode_names.len() {
        union(soa.diode_anode[i], soa.diode_cathode[i], &mut parent);
    }
    for i in 0..soa.mos_names.len() {
        union(soa.mos_drain[i], soa.mos_source[i], &mut parent);
    }
    for i in 0..soa.ctl_names.len() {
        union(soa.ctl_n_pos[i], soa.ctl_n_neg[i], &mut parent);
    }
    for (a, c) in soa.ext.junction_terminals() {
        union(a, c, &mut parent);
    }
    let mut injected = vec![0.0_f64; n];
    let mut magnitude = vec![0.0_f64; n];
    let mut indeterminate = vec![false; n];
    for i in 0..soa.isrc_names.len() {
        let (p, q, v) = (soa.isrc_n_pos[i], soa.isrc_n_neg[i], soa.isrc_values[i]);
        let (rp, rq) = (find(&mut parent, p), find(&mut parent, q));
        // current flows n+ -> n- THROUGH the source: it leaves n+ and enters n-
        injected[rp] -= v;
        injected[rq] += v;
        magnitude[rp] += v.abs();
        magnitude[rq] += v.abs();
    }
    let current_outputs = (0..soa.vccs_names.len())
        .map(|i| (soa.vccs_n_pos[i], soa.vccs_n_neg[i]))
        .chain(soa.ctl_current_nodes.iter().copied());
    for (p, q) in current_outputs {
        let (rp, rq) = (find(&mut parent, p), find(&mut parent, q));
        indeterminate[rp] = true;
        indeterminate[rq] = true;
    }
    let ground = find(&mut parent, 0);
    for root in 0..n {
        if root == ground || indeterminate[root] || magnitude[root] == 0.0 {
            continue;
        }
        if find(&mut parent, root) != root {
            continue;
        }
        if injected[root].abs() > 1e-9 * magnitude[root] {
            let nodes: Vec<usize> = (1..n).filter(|&k| find(&mut parent, k) == root).collect();
            return Some((nodes, injected[root]));
        }
    }
    None
}

/// Temperatures (kelvin) of one device: its own and the nominal one of its model.
fn device_temperatures(
    netlist: &Netlist,
    name: &str,
    temp: Option<f64>,
    dtemp: Option<f64>,
    model_tnom: Option<f64>,
) -> SimResult<(f64, f64)> {
    use crate::temperature::{DEFAULT_TEMP_C, KELVIN};
    let circuit = netlist.temp.unwrap_or(DEFAULT_TEMP_C);
    let t = temp.unwrap_or(circuit) + dtemp.unwrap_or(0.0) + KELVIN;
    let tnom = model_tnom.or(netlist.tnom).unwrap_or(DEFAULT_TEMP_C) + KELVIN;
    if !(t > 0.0 && t.is_finite() && tnom > 0.0 && tnom.is_finite()) {
        return Err(SimError::TopologyError(format!(
            "`{name}`: device temperature {:.2} K / nominal {:.2} K is not above absolute zero",
            t, tnom
        )));
    }
    Ok((t, tnom))
}

/// Diode parameters at the device temperature: `(IS, N_eff, CJO, VJ)` per unit area, where `N_eff * Vt(circuit)`
/// equals `N * Vt(device)` (the kernels share one thermal voltage).
fn diode_at_temperature(
    netlist: &Netlist,
    d: &crate::netlist::DiodeInstance,
    m: &crate::netlist::DiodeModelParams,
) -> SimResult<(f64, f64, f64, f64)> {
    use crate::temperature as tk;
    let (t, tnom) = device_temperatures(netlist, &d.name, d.temp, d.dtemp, m.tnom)?;
    let t_circuit = netlist.temp.unwrap_or(tk::DEFAULT_TEMP_C) + tk::KELVIN;
    let is = tk::diode_saturation_current(
        m.is,
        m.n,
        m.eg.unwrap_or(1.11),
        m.xti.unwrap_or(3.0),
        t,
        tnom,
    );
    let n_eff = if t == t_circuit {
        m.n
    } else {
        m.n * (t / t_circuit)
    };
    let cjo = tk::junction_capacitance(m.cjo, m.m, m.vj, t, tnom);
    let vj = tk::junction_potential(m.vj, t, tnom);
    Ok((is, n_eff, cjo, vj))
}

/// Appends a device-internal node and returns its 1-based id.
fn internal_node(node_names: &mut Vec<String>, name: String) -> usize {
    node_names.push(name);
    node_names.len() - 1
}

/// Level-1 parameters of one MOSFET as its kernel sees them.
#[derive(Clone, Debug, Default)]
struct MosCompiled {
    /// `KP`, `VTO`, `PHI` at the device temperature.
    kp: f64,
    vto: f64,
    gamma: f64,
    phi: f64,
    phi_nominal: f64,
    /// Effective channel length `L - 2 LD`.
    leff: f64,
    temperature: f64,
}

/// Level-1 electrical parameters after the derivations from the process parameters.
struct Level1Params {
    kp: f64,
    vto: f64,
    gamma: f64,
    phi: f64,
    /// Oxide capacitance per area (F/m^2; 0 without `TOX`).
    cox: f64,
}

/// Vacuum permittivity of the oxide / silicon permittivities (F/m).
const EPS0: f64 = 8.854214871e-12;
/// Elementary charge (C) and intrinsic carrier density of silicon (m^-3).
const Q_ELECTRON: f64 = 1.602_176_634e-19;
const NI_SILICON: f64 = 1.45e16;

/// SPICE level-1 derivations at the nominal temperature `tnom` (K): `KP = UO Cox` without `KP` (with `TOX`);
/// with `NSUB` and `TOX`, `PHI = 2 Vt ln(NSUB / ni)`, `GAMMA = sqrt(2 q eps_si NSUB) / Cox` and
/// `VTO = Vfb + type (PHI + GAMMA sqrt(PHI))` from the gate work-function difference (`TPG`) and `NSS`, each only
/// where the card does not give it.
fn level1_effective(m: &crate::netlist::MosfetModelParams, tnom: f64) -> SimResult<Level1Params> {
    let pr = &m.process;
    let cox = pr.tox.map_or(0.0, |tox| 3.9 * EPS0 / tox);
    let kp = if pr.kp_given || cox == 0.0 {
        m.kp
    } else {
        pr.uo.unwrap_or(600.0) * 1e-4 * cox
    };
    let (mut vto, mut gamma, mut phi) = (m.vto, m.gamma, m.phi);
    if let Some(nsub) = pr.nsub.filter(|_| cox > 0.0) {
        let n = nsub * 1e6;
        if !pr.phi_given {
            phi = 2.0 * crate::temperature::thermal_voltage(tnom) * (n / NI_SILICON).ln();
            if phi <= 0.0 {
                return Err(SimError::TopologyError(format!(
                    "MOSFET model `{}`: NSUB={nsub:e} cm^-3 is below the intrinsic density (PHI={phi:.4})",
                    m.name
                )));
            }
        }
        if !pr.gamma_given {
            gamma = (2.0 * Q_ELECTRON * 11.7 * EPS0 * n).sqrt() / cox;
        }
        if !pr.vto_given {
            let typ = m.mos_type.sign();
            let tpg = pr.tpg.unwrap_or(1.0);
            let half_gap = 0.5 * crate::temperature::silicon_gap(tnom);
            let fermi = typ * 0.5 * phi;
            // gate - substrate work-function difference: poly gate of the opposite / same type, or aluminium
            let phi_ms = if tpg == 0.0 {
                -0.05 - half_gap - fermi
            } else {
                -tpg.signum() * typ * half_gap - fermi
            };
            let vfb = phi_ms - pr.nss.unwrap_or(0.0) * 1e4 * Q_ELECTRON / cox;
            vto = vfb + typ * (phi + gamma * phi.sqrt());
        }
    }
    Ok(Level1Params {
        kp,
        vto,
        gamma,
        phi,
        cox,
    })
}

/// Compiles a parsed [`Netlist`] into a 64-byte aligned [`DeviceSoaBuffer`] and a conflict-free
/// [`CscGatherTable`] + [`CscMatrix`].
pub fn compile_netlist(netlist: &Netlist) -> SimResult<CompiledTopology> {
    let mut netlist_expanded = netlist.clone();
    // device-internal nodes follow the circuit nodes: `node_names[num_nodes(netlist)+1..]`
    let mut node_names = netlist.node_names.clone();
    let vt = crate::temperature::thermal_voltage(
        netlist.temp.unwrap_or(crate::temperature::DEFAULT_TEMP_C) + crate::temperature::KELVIN,
    );
    let mut ext = ExtDevices::default();

    // Diodes beyond the ideal Shockley equation become full junctions (behind an internal node for RS)
    let mut plain_diodes = Vec::with_capacity(netlist.diodes.len());
    for d in &netlist.diodes {
        let m = netlist
            .diode_models
            .get(&d.model)
            .cloned()
            .unwrap_or_default();
        if !m.is_extended() {
            plain_diodes.push(d.clone());
            continue;
        }
        let (is_t, n_eff, cjo_t, vj_t) = diode_at_temperature(netlist, d, &m)?;
        let mut anode = d.n_anode;
        if m.rs > 0.0 {
            anode = internal_node(&mut node_names, format!("{}#internal", d.name));
            netlist_expanded.resistors.push(ResistorInstance {
                name: format!("{}#rs", d.name),
                n_pos: d.n_anode,
                n_neg: anode,
                resistance: m.rs / d.area,
            });
        }
        let is = is_t * d.area;
        ext.junctions.push(Junction {
            name: d.name.clone(),
            anode,
            cathode: d.n_cathode,
            is,
            n: n_eff,
            xbv: Junction::fit_breakdown(m.bv, m.ibv.unwrap_or(1e-3) * d.area, is, n_eff, vt),
            ikf: m.ikf.map_or(f64::INFINITY, |v| v * d.area),
            isr: m.isr.map_or(0.0, |v| v * d.area),
            nr: m.nr.unwrap_or(2.0),
            cj0: cjo_t * d.area,
            vj: vj_t,
            m: m.m,
            fc: m.fc.unwrap_or(0.5),
            tt: m.tt,
            cubic_reverse: true,
            ..Default::default()
        });
    }
    netlist_expanded.diodes = plain_diodes;

    // Level-1 MOSFETs: derived parameters, series resistances, bulk junctions, gate capacitances
    let mut mos_params: Vec<MosCompiled> = Vec::with_capacity(netlist.mosfets.len());
    for (mos_idx, mos) in netlist_expanded.mosfets.iter_mut().enumerate() {
        let m = &netlist
            .mosfet_models
            .get(&mos.model)
            .cloned()
            .unwrap_or_default();
        let (t, tnom) = device_temperatures(netlist, &mos.name, mos.temp, mos.dtemp, m.tnom)?;
        let lv = level1_effective(m, tnom)?;
        let par = &m.parasitics;
        let w = mos.w.unwrap_or(m.w_default) * mos.m;
        let leff = mos.l.unwrap_or(m.l_default) - 2.0 * par.ld.unwrap_or(0.0);
        if leff <= 0.0 && mos.l.unwrap_or(m.l_default) != 0.0 {
            return Err(SimError::TopologyError(format!(
                "MOSFET `{}`: effective channel length L - 2 LD = {leff:e} m is not positive",
                mos.name
            )));
        }
        let rsh = par.rsh.unwrap_or(0.0);
        let series = |r: Option<f64>, squares: Option<f64>| {
            r.filter(|&r| r > 0.0)
                .or_else(|| squares.map(|n| n * rsh).filter(|&r| r > 0.0))
        };
        let (n_drain, n_source) = (mos.n_drain, mos.n_source);
        if let Some(rd) = series(par.rd, mos.geom.nrd) {
            mos.n_drain = internal_node(&mut node_names, format!("{}#drain", mos.name));
            netlist_expanded.resistors.push(ResistorInstance {
                name: format!("{}#rd", mos.name),
                n_pos: n_drain,
                n_neg: mos.n_drain,
                resistance: rd / mos.m,
            });
        }
        if let Some(rs) = series(par.rs, mos.geom.nrs) {
            mos.n_source = internal_node(&mut node_names, format!("{}#source", mos.name));
            netlist_expanded.resistors.push(ResistorInstance {
                name: format!("{}#rs", mos.name),
                n_pos: n_source,
                n_neg: mos.n_source,
                resistance: rs / mos.m,
            });
        }
        // bulk-drain and bulk-source junctions (the diode points from the bulk for an NMOS)
        let is_bulk = par.is.unwrap_or(1e-14);
        let js = par.js.unwrap_or(0.0);
        let pb = par.pb.unwrap_or(0.8);
        let t_circuit =
            netlist.temp.unwrap_or(crate::temperature::DEFAULT_TEMP_C) + crate::temperature::KELVIN;
        let n_eff = if t == t_circuit { 1.0 } else { t / t_circuit };
        let pb_t = crate::temperature::junction_potential(pb, t, tnom);
        let (mj, mjsw) = (par.mj.unwrap_or(0.5), par.mjsw.unwrap_or(0.33));
        let cap_t = |c: f64, grading: f64| {
            crate::temperature::junction_capacitance(c, grading, pb, t, tnom)
        };
        let nmos = m.mos_type == crate::netlist::MosfetType::Nmos;
        for (node, other, area, perim, cb) in [
            (mos.n_drain, mos.n_source, mos.geom.ad, mos.geom.pd, par.cbd),
            (
                mos.n_source,
                mos.n_drain,
                mos.geom.as_,
                mos.geom.ps,
                par.cbs,
            ),
        ] {
            if node == mos.n_bulk {
                continue;
            }
            // with the bulk tied to the other channel terminal the junction lies across drain-source
            let fold_mos = (other == mos.n_bulk).then_some(mos_idx);
            let area = area.unwrap_or(0.0);
            let is0 = if js > 0.0 && area > 0.0 {
                js * area
            } else {
                is_bulk
            };
            let is_t =
                crate::temperature::diode_saturation_current(is0, 1.0, 1.11, 3.0, t, tnom) * mos.m;
            let cj0 = cb.unwrap_or(par.cj.unwrap_or(0.0) * area) * mos.m;
            let cjsw = par.cjsw.unwrap_or(0.0) * perim.unwrap_or(0.0) * mos.m;
            let (anode, cathode) = if nmos {
                (mos.n_bulk, node)
            } else {
                (node, mos.n_bulk)
            };
            let fold_sign = if anode == mos.n_drain { 1.0 } else { -1.0 };
            ext.junctions.push(Junction {
                name: format!("{}#bulk", mos.name),
                anode,
                cathode,
                is: is_t,
                n: n_eff,
                xbv: f64::INFINITY,
                ikf: f64::INFINITY,
                nr: 2.0,
                cj0: cap_t(cj0, mj),
                vj: pb_t,
                m: mj,
                fc: par.fc.unwrap_or(0.5),
                cjsw: cap_t(cjsw, mjsw),
                mjsw,
                fold_mos,
                fold_sign,
                ..Default::default()
            });
        }
        let (kp_t, vto_t, phi_t) = crate::temperature::level1_at(
            lv.kp,
            lv.vto,
            lv.phi,
            lv.gamma,
            m.mos_type.sign(),
            t,
            tnom,
        );
        if lv.cox > 0.0 {
            ext.meyer.push(MeyerCaps {
                name: mos.name.clone(),
                drain: mos.n_drain,
                gate: mos.n_gate,
                source: mos.n_source,
                bulk: mos.n_bulk,
                polarity: m.mos_type.sign(),
                vto: m.mos_type.sign() * vto_t,
                gamma: lv.gamma,
                phi: phi_t,
                cox: lv.cox * w * leff,
                ..Default::default()
            });
        }
        mos_params.push(MosCompiled {
            kp: kp_t,
            vto: vto_t,
            gamma: lv.gamma,
            phi: phi_t,
            phi_nominal: lv.phi,
            leff,
            temperature: t,
        });
    }

    // Overlap capacitances: CGSO / CGDO per unit width, CGBO per unit (effective) length
    for (mos, mc) in netlist_expanded.mosfets.clone().iter().zip(&mos_params) {
        if let Some(m) = netlist.mosfet_models.get(&mos.model) {
            let w = mos.w.unwrap_or(m.w_default) * mos.m;
            for (tag, other, c) in [
                ("gs", mos.n_source, m.cgso * w),
                ("gd", mos.n_drain, m.cgdo * w),
                ("gb", mos.n_bulk, m.cgbo * mc.leff * mos.m),
            ] {
                if c > 0.0 {
                    netlist_expanded
                        .capacitors
                        .push(crate::netlist::CapacitorInstance {
                            name: format!("c_{tag}_{}", mos.name),
                            n_pos: mos.n_gate,
                            n_neg: other,
                            capacitance: c,
                            ic: None,
                        });
                }
            }
        }
    }

    let num_nodes = node_names.len() - 1;
    let mna_dim = num_nodes
        + netlist_expanded.v_sources.len()
        + netlist_expanded.inductors.len()
        + netlist_expanded.vcvs.len()
        + netlist_expanded.ccvs.len();
    if mna_dim == 0 {
        return Err(SimError::TopologyError(
            "Circuit contains no non-ground nodes or branches".to_string(),
        ));
    }

    let mut soa = DeviceSoaBuffer {
        num_nodes,
        mna_dim,
        gmin: netlist.gmin.unwrap_or(1e-12),
        gmin_nominal: netlist.gmin.unwrap_or(1e-12),
        // k T / q at the circuit temperature (exactly the ngspice-47 value 0.025864917007155805 at 27 C)
        temp_vt: crate::temperature::thermal_voltage(
            netlist.temp.unwrap_or(crate::temperature::DEFAULT_TEMP_C) + crate::temperature::KELVIN,
        ),
        is_transient: false,
        transient_g_scale: 0.0,
        node_names,
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
        let g = DeviceSoaBuffer::resistor_conductance(&r.name, r.resistance);
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
        soa.cap_ic.push(c.ic.unwrap_or(f64::NAN));

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
        soa.ind_ic.push(ind.ic.unwrap_or(f64::NAN));

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

    // 5b. Controlled sources. `E` / `H` rows follow the inductor branches: `v(n+) - v(n-) - f = 0`.
    let vsrc_branch = |name: &str| -> SimResult<usize> {
        netlist_expanded
            .v_sources
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(name))
            .map(|k| num_nodes + k)
            .ok_or_else(|| {
                SimError::TopologyError(format!(
                    "`{name}` is not an independent voltage source of the circuit"
                ))
            })
    };
    let mut br = num_nodes + netlist_expanded.v_sources.len() + netlist_expanded.inductors.len();
    let mut push_branch = |soa: &mut DeviceSoaBuffer,
                           stamps: &mut Vec<(usize, usize, u32, f64, bool)>,
                           name: &str,
                           n_pos: usize,
                           n_neg: usize|
     -> usize {
        let one = soa.cond_buf.len() as u32;
        soa.cond_buf.push(1.0);
        if n_pos > 0 {
            stamps.push((n_pos - 1, br, one, 1.0, true));
            stamps.push((br, n_pos - 1, one, 1.0, true));
        }
        if n_neg > 0 {
            stamps.push((n_neg - 1, br, one, -1.0, true));
            stamps.push((br, n_neg - 1, one, -1.0, true));
        }
        soa.ctl_names.push(name.to_string());
        soa.ctl_n_pos.push(n_pos);
        soa.ctl_n_neg.push(n_neg);
        soa.ctl_branch_idx.push(br);
        br += 1;
        br - 1
    };
    for e in &netlist_expanded.vcvs {
        let row = push_branch(&mut soa, &mut matrix_stamps, &e.name, e.n_pos, e.n_neg);
        let linear = e.coeffs.len() <= 2;
        let g_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(e.coeffs.get(1).copied().unwrap_or(0.0));
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(e.coeffs.first().copied().unwrap_or(0.0));
        if e.nc_pos > 0 {
            matrix_stamps.push((row, e.nc_pos - 1, g_idx, -1.0, linear));
        }
        if e.nc_neg > 0 {
            matrix_stamps.push((row, e.nc_neg - 1, g_idx, 1.0, linear));
        }
        rhs_stamps.push((row, rhs_idx, 1.0));
        if !linear {
            ext.poly.push(PolySource {
                name: e.name.clone(),
                nc_pos: e.nc_pos,
                nc_neg: e.nc_neg,
                coeffs: e.coeffs.clone(),
                cond_buf_idx: g_idx,
                rhs_buf_idx: rhs_idx,
            });
        }
    }
    for h in &netlist_expanded.ccvs {
        let ctrl = vsrc_branch(&h.control)?;
        let row = push_branch(&mut soa, &mut matrix_stamps, &h.name, h.n_pos, h.n_neg);
        let r_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(h.r);
        matrix_stamps.push((row, ctrl, r_idx, -1.0, true));
    }
    // `F`: `gain * i(control)` leaves n+ and enters n-
    for f in &netlist_expanded.cccs {
        let ctrl = vsrc_branch(&f.control)?;
        let g_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(f.gain);
        if f.n_pos > 0 {
            matrix_stamps.push((f.n_pos - 1, ctrl, g_idx, 1.0, true));
        }
        if f.n_neg > 0 {
            matrix_stamps.push((f.n_neg - 1, ctrl, g_idx, -1.0, true));
        }
        soa.ctl_current_nodes.push((f.n_pos, f.n_neg));
    }
    // `G ... POLY(1)`: the VCCS stamps with a slope and an offset current
    for g in &netlist_expanded.poly_vccs {
        let linear = g.coeffs.len() <= 2;
        let g_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(g.coeffs.get(1).copied().unwrap_or(0.0));
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(g.coeffs.first().copied().unwrap_or(0.0));
        for (row, sign) in [(g.n_pos, 1.0), (g.n_neg, -1.0)] {
            if row == 0 {
                continue;
            }
            if g.nc_pos > 0 {
                matrix_stamps.push((row - 1, g.nc_pos - 1, g_idx, sign, linear));
            }
            if g.nc_neg > 0 {
                matrix_stamps.push((row - 1, g.nc_neg - 1, g_idx, -sign, linear));
            }
            rhs_stamps.push((row - 1, rhs_idx, -sign));
        }
        soa.ctl_current_nodes.push((g.n_pos, g.n_neg));
        if !linear {
            ext.poly.push(PolySource {
                name: g.name.clone(),
                nc_pos: g.nc_pos,
                nc_neg: g.nc_neg,
                coeffs: g.coeffs.clone(),
                cond_buf_idx: g_idx,
                rhs_buf_idx: rhs_idx,
            });
        }
    }
    // `K`: `-M a0` between the two branch equations (zero at DC)
    for k in &netlist_expanded.mutuals {
        let find = |name: &str| {
            netlist_expanded
                .inductors
                .iter()
                .position(|l| l.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    SimError::TopologyError(format!(
                        "{}: `{name}` is not an inductor of the circuit",
                        k.name
                    ))
                })
        };
        let (a, b) = (find(&k.l1)?, find(&k.l2)?);
        if a == b {
            return Err(SimError::TopologyError(format!(
                "{}: an inductor cannot be coupled to itself",
                k.name
            )));
        }
        let m = k.k * (soa.ind_inductance[a] * soa.ind_inductance[b]).sqrt();
        let idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let (br_a, br_b) = (soa.ind_branch_idx[a], soa.ind_branch_idx[b]);
        matrix_stamps.push((br_a, br_b, idx, -1.0, false));
        matrix_stamps.push((br_b, br_a, idx, -1.0, false));
        ext.mutual.push(Mutual {
            name: k.name.clone(),
            ind_a: a,
            ind_b: b,
            m,
            cond_buf_idx: idx,
        });
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
        let (is_t, n_eff, cjo_t, vj_t) = diode_at_temperature(&netlist_expanded, d, &m)?;
        soa.diode_is.push(is_t * d.area);
        soa.diode_n.push(n_eff);
        soa.diode_area.push(d.area);
        soa.diode_cjo.push(cjo_t * d.area);
        soa.diode_vj.push(vj_t);
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
    for (mos, mc) in netlist_expanded.mosfets.iter().zip(&mos_params) {
        let m = netlist_expanded
            .mosfet_models
            .get(&mos.model)
            .cloned()
            .unwrap_or_default();
        let l_val = mos.l.unwrap_or(m.l_default);
        if l_val == 0.0 {
            return Err(SimError::TopologyError(format!(
                "MOSFET `{}` has zero channel length L=0",
                mos.name
            )));
        }
        if mc.phi_nominal <= 0.0 {
            return Err(SimError::TopologyError(format!(
                "MOSFET model `{}` has non-positive surface potential PHI={}",
                m.name, mc.phi_nominal
            )));
        }
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
        if mc.phi <= 0.0 {
            return Err(SimError::TopologyError(format!(
                "MOSFET `{}`: surface potential PHI={:.4} V at {:.2} K is not positive",
                mos.name, mc.phi, mc.temperature
            )));
        }
        soa.mos_kp.push(mc.kp);
        soa.mos_w.push(mos.w.unwrap_or(m.w_default) * mos.m);
        soa.mos_l.push(mc.leff);
        soa.mos_vto.push(mc.vto);
        soa.mos_gamma.push(mc.gamma);
        soa.mos_phi.push(mc.phi);
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

    // 8b. Full junctions and Meyer gate capacitances: a conductance and a Norton current per branch
    let mut full = Vec::with_capacity(ext.junctions.len());
    for mut j in std::mem::take(&mut ext.junctions) {
        j.prepare(soa.temp_vt);
        if let Some(k) = j.fold_mos {
            j.cond_buf_idx = soa.mos_gds_buf_idx[k];
            j.rhs_buf_idx = soa.mos_rhs_buf_idx[k];
        } else {
            j.cond_buf_idx = soa.cond_buf.len() as u32;
            soa.cond_buf.push(0.0);
            j.rhs_buf_idx = soa.rhs_buf.len() as u32;
            soa.rhs_buf.push(0.0);
            add_2pin_conductance(
                j.anode,
                j.cathode,
                j.cond_buf_idx,
                false,
                &mut matrix_stamps,
            );
            if j.anode > 0 {
                rhs_stamps.push((j.anode - 1, j.rhs_buf_idx, -1.0));
            }
            if j.cathode > 0 {
                rhs_stamps.push((j.cathode - 1, j.rhs_buf_idx, 1.0));
            }
        }
        if j.is_plain() {
            ext.bulk.push(&j);
        } else {
            full.push(j);
        }
    }
    ext.junctions = full;
    for mc in &mut ext.meyer {
        for (k, other) in [mc.source, mc.drain, mc.bulk].into_iter().enumerate() {
            mc.cond_buf_idx[k] = soa.cond_buf.len() as u32;
            soa.cond_buf.push(0.0);
            mc.rhs_buf_idx[k] = soa.rhs_buf.len() as u32;
            soa.rhs_buf.push(0.0);
            add_2pin_conductance(
                mc.gate,
                other,
                mc.cond_buf_idx[k],
                false,
                &mut matrix_stamps,
            );
            if mc.gate > 0 {
                rhs_stamps.push((mc.gate - 1, mc.rhs_buf_idx[k], -1.0));
            }
            if other > 0 {
                rhs_stamps.push((other - 1, mc.rhs_buf_idx[k], 1.0));
            }
        }
    }

    // 9. `.ic` node clamps: a conductance to ground plus a Norton current, both zero until the transient start
    //    point is solved (`set_ic_clamps`), so they never touch `.op`, `.dc` or the transient steps.
    let mut ic_entries: Vec<(usize, f64)> = netlist
        .initial_conditions
        .iter()
        .filter(|(&node, _)| node > 0 && node <= num_nodes)
        .map(|(&node, &v)| (node, v))
        .collect();
    ic_entries.sort_by_key(|&(node, _)| node);
    for (node, v) in ic_entries {
        let cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);
        soa.ic_nodes.push(node);
        soa.ic_values.push(v);
        soa.ic_cond_buf_idx.push(cond_idx);
        soa.ic_rhs_buf_idx.push(rhs_idx);
        matrix_stamps.push((node - 1, node - 1, cond_idx, 1.0, false));
        rhs_stamps.push((node - 1, rhs_idx, 1.0));
    }

    // 10. `.nodeset` node clamps: the same Norton form, active only while the first phase of an operating point
    //     is solved (`set_nodeset_clamps`); the second phase starts from that solution without them.
    let mut nodeset_entries: Vec<(usize, f64)> = netlist
        .nodesets
        .iter()
        .filter(|(&node, _)| node > 0 && node <= num_nodes)
        .map(|(&node, &v)| (node, v))
        .collect();
    nodeset_entries.sort_by_key(|&(node, _)| node);
    for (node, v) in nodeset_entries {
        let cond_idx = soa.cond_buf.len() as u32;
        soa.cond_buf.push(0.0);
        let rhs_idx = soa.rhs_buf.len() as u32;
        soa.rhs_buf.push(0.0);
        soa.nodeset_nodes.push(node);
        soa.nodeset_values.push(v);
        soa.nodeset_cond_buf_idx.push(cond_idx);
        soa.nodeset_rhs_buf_idx.push(rhs_idx);
        matrix_stamps.push((node - 1, node - 1, cond_idx, 1.0, false));
        rhs_stamps.push((node - 1, rhs_idx, 1.0));
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
    let junction_nodes = ext.junction_terminals().flat_map(|(a, c)| [a, c]);
    let branch_nodes = (0..soa.ctl_names.len()).flat_map(|i| [soa.ctl_n_pos[i], soa.ctl_n_neg[i]]);
    for node in junction_nodes.chain(branch_nodes) {
        if node > 0 {
            node_has_dc_device[node - 1] = true;
        }
    }
    let mut dc_floating_diag_idx = Vec::new();
    for (d, &has_dc) in node_has_dc_device.iter().enumerate() {
        if !has_dc {
            let diag = jacobian.diag_idx[d];
            if diag < nnz {
                static_linear_csc_values[diag] += 1e-12;
                dc_floating_diag_idx.push(diag);
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
    soa.dc_floating_diag_idx = dc_floating_diag_idx;
    soa.linear_dirty = false;
    soa.ext = ext;

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
        jagged_nnz: JaggedTerms::build(&active_nnz, &nnz_terms),
        jagged_rhs: JaggedTerms::build(&active_rhs, &rhs_terms),
        nnz_ptr,
        nnz_terms,
        active_nnz,
        rhs_ptr,
        rhs_terms,
        active_rhs,
    }
}
