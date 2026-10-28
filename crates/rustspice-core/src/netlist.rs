//! High-speed zero-copy SPICE netlist parser and programmatic [`CircuitBuilder`] API.

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::diag::{
    audit_element_tail, audit_ic, audit_ignored_dot_card, audit_model_params, audit_nodes,
    audit_number, audit_tran, code, DiagSink, Diagnostic, Elem, KvPair,
};
use crate::error::{SimError, SimResult};
use crate::limits::ResourceLimits;

/// Length of the numeric prefix (`digits . e +-`) that [`parse_spice_number`] reads; the rest is the suffix.
pub(crate) fn number_prefix_len(bytes: &[u8]) -> usize {
    let mut num_end = 0;
    let mut seen_e = false;
    while num_end < bytes.len() {
        let b = bytes[num_end];
        if b.is_ascii_digit() || b == b'.' {
            num_end += 1;
        } else if (b == b'+' || b == b'-') && (num_end == 0 || seen_e) {
            num_end += 1;
            seen_e = false;
        } else if (b == b'e' || b == b'E')
            && !seen_e
            && num_end + 1 < bytes.len()
            && (bytes[num_end + 1].is_ascii_digit()
                || bytes[num_end + 1] == b'+'
                || bytes[num_end + 1] == b'-')
        {
            seen_e = true;
            num_end += 1;
        } else {
            break;
        }
    }
    num_end
}

/// Parses a SPICE numeric literal with engineering unit suffixes (`f`, `p`, `n`, `u`, `m`,
/// `mil`, `k`, `meg`, `g`, `t`).
pub fn parse_spice_number(raw: &str, params: &FxHashMap<String, f64>) -> Option<f64> {
    let trimmed = raw
        .trim()
        .trim_matches(|c| c == '{' || c == '}' || c == '\'' || c == '"');
    if trimmed.is_empty() {
        return None;
    }
    if let Some(&val) = params.get(&trimmed.to_ascii_lowercase()) {
        return Some(val);
    }

    let num_end = number_prefix_len(trimmed.as_bytes());
    if num_end == 0 {
        return None;
    }
    let base: f64 = trimmed[..num_end].parse().ok()?;
    let suffix = trimmed[num_end..].to_ascii_lowercase();
    let mult = if suffix.is_empty() {
        1.0
    } else if suffix.starts_with("meg") || suffix.starts_with('x') {
        1e6
    } else if suffix.starts_with("mil") {
        25.4e-6
    } else {
        match suffix.as_bytes()[0] {
            b't' => 1e12,
            b'g' => 1e9,
            b'k' => 1e3,
            b'm' => 1e-3,
            b'u' => 1e-6,
            b'n' => 1e-9,
            b'p' => 1e-12,
            b'f' => 1e-15,
            b'a' => 1e-18,
            _ => 1.0,
        }
    };
    Some(base * mult)
}

/// Time-dependent waveform specification for independent voltage and current sources.
///
/// Timing fields that the card leaves to their SPICE defaults are `NaN` until [`SourceWaveform::resolve_defaults`]
/// fills them from the `.tran` card (PULSE `tr`/`tf` = tstep, `pw`/`per` = tstop; SIN/SFFM frequencies = 1/tstop;
/// EXP `tau1`/`tau2` = tstep, `td2` = td1 + tstep). `dc_val` is the value used by `.op` / `.dc`: the explicit
/// `DC <v>` (or leading value) of the card, otherwise the waveform's value at `t = 0` (0 for SFFM, as in ngspice).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SourceWaveform {
    /// Constant DC value.
    Dc(f64),
    /// Periodic trapezoidal pulse: `PULSE(v1 v2 td tr tf pw per)`.
    Pulse {
        dc_val: f64,
        v1: f64,
        v2: f64,
        td: f64,
        tr: f64,
        tf: f64,
        pw: f64,
        per: f64,
    },
    /// Damped sinusoid: `SIN(vo va freq td theta phase)` (phase in degrees).
    Sin {
        dc_val: f64,
        vo: f64,
        va: f64,
        freq: f64,
        td: f64,
        theta: f64,
        #[serde(default)]
        phase: f64,
    },
    /// Rise / fall exponentials: `EXP(v1 v2 td1 tau1 td2 tau2)`.
    Exp {
        dc_val: f64,
        v1: f64,
        v2: f64,
        td1: f64,
        tau1: f64,
        td2: f64,
        tau2: f64,
    },
    /// Piecewise linear `PWL(t1 v1 t2 v2 ...) [r=<t>] [td=<t>]`: `repeat` restarts the sequence at that time
    /// point after the last one, `delay` shifts the whole waveform.
    Pwl {
        dc_val: f64,
        points: Vec<(f64, f64)>,
        repeat: Option<f64>,
        delay: f64,
    },
    /// Single-frequency FM `SFFM(vo va fc mdi fs phasec phases)` (phases in degrees).
    Sffm {
        dc_val: f64,
        vo: f64,
        va: f64,
        fc: f64,
        mdi: f64,
        fs: f64,
        phasec: f64,
        phases: f64,
    },
}

impl SourceWaveform {
    /// Returns the source value during DC operating point analysis.
    #[inline]
    pub fn dc_value(&self) -> f64 {
        match self {
            Self::Dc(v) => *v,
            Self::Pulse { dc_val, .. }
            | Self::Sin { dc_val, .. }
            | Self::Exp { dc_val, .. }
            | Self::Pwl { dc_val, .. }
            | Self::Sffm { dc_val, .. } => *dc_val,
        }
    }

    /// Fills the timing fields left to their SPICE defaults (`NaN`) from the `.tran` step and stop time.
    pub fn resolve_defaults(&mut self, tstep: f64, tstop: f64) {
        let fill = |x: &mut f64, v: f64| {
            if x.is_nan() {
                *x = v;
            }
        };
        match self {
            Self::Pulse {
                tr, tf, pw, per, ..
            } => {
                fill(tr, tstep);
                fill(tf, tstep);
                fill(pw, tstop);
                fill(per, tstop);
            }
            Self::Sin { freq, .. } => fill(freq, 1.0 / tstop),
            Self::Exp {
                td1,
                tau1,
                td2,
                tau2,
                ..
            } => {
                fill(tau1, tstep);
                fill(td2, *td1 + tstep);
                fill(tau2, tstep);
            }
            Self::Sffm { fc, fs, .. } => {
                fill(fc, 1.0 / tstop);
                fill(fs, 1.0 / tstop);
            }
            Self::Dc(_) | Self::Pwl { .. } => {}
        }
    }

    /// Evaluates the instantaneous source value at simulation time `t` (in seconds).
    /// Note: at `t == 0.0` (initial transient operating point), matches SPICE `v1` / `vo`.
    #[inline]
    pub fn evaluate_at(&self, t: f64) -> f64 {
        const DEG: f64 = std::f64::consts::PI / 180.0;
        match self {
            Self::Dc(v) => *v,
            &Self::Pulse {
                v1,
                v2,
                td,
                tr,
                tf,
                pw,
                per,
                ..
            } => {
                if t <= td {
                    return v1;
                }
                let mut rel_t = t - td;
                if per > 0.0 {
                    rel_t %= per;
                }
                let tr_eff = tr.max(0.0);
                let tf_eff = tf.max(0.0);
                let pw_eff = pw.max(0.0);
                if rel_t < tr_eff {
                    if tr_eff > 0.0 {
                        v1 + (v2 - v1) * (rel_t / tr_eff)
                    } else {
                        v2
                    }
                } else if rel_t < tr_eff + pw_eff {
                    v2
                } else if rel_t < tr_eff + pw_eff + tf_eff {
                    if tf_eff > 0.0 {
                        v2 + (v1 - v2) * ((rel_t - tr_eff - pw_eff) / tf_eff)
                    } else {
                        v1
                    }
                } else {
                    v1
                }
            }
            &Self::Sin {
                vo,
                va,
                freq,
                td,
                theta,
                phase,
                ..
            } => {
                if t <= td {
                    vo + va * (phase * DEG).sin()
                } else {
                    let dt = t - td;
                    vo + va
                        * (-dt * theta).exp()
                        * (2.0 * std::f64::consts::PI * freq * dt + phase * DEG).sin()
                }
            }
            &Self::Exp {
                v1,
                v2,
                td1,
                tau1,
                td2,
                tau2,
                ..
            } => {
                let mut v = v1;
                if t > td1 {
                    v += (v2 - v1) * (1.0 - (-(t - td1) / tau1).exp());
                }
                if t > td2 {
                    v += (v1 - v2) * (1.0 - (-(t - td2) / tau2).exp());
                }
                v
            }
            Self::Pwl {
                points,
                repeat,
                delay,
                ..
            } => {
                let Some(&(t_first, v_first)) = points.first() else {
                    return 0.0;
                };
                let &(t_last, v_last) = points.last().unwrap_or(&(t_first, v_first));
                let mut tau = t - delay;
                if tau <= t_first {
                    return v_first;
                }
                if tau > t_last {
                    match repeat {
                        Some(r) if *r < t_last => {
                            tau = r + (tau - t_last) % (t_last - r);
                        }
                        _ => return v_last,
                    }
                }
                // first point at or after tau
                let k = points.partition_point(|p| p.0 < tau);
                if k == 0 {
                    return points[0].1;
                }
                let (ta, va) = points[k - 1];
                let (tb, vb) = points[k.min(points.len() - 1)];
                if tb > ta {
                    va + (vb - va) * (tau - ta) / (tb - ta)
                } else {
                    vb
                }
            }
            &Self::Sffm {
                vo,
                va,
                fc,
                mdi,
                fs,
                phasec,
                phases,
                ..
            } => {
                let w = 2.0 * std::f64::consts::PI;
                vo + va
                    * (w * fc * t + phasec * DEG + mdi * (w * fs * t + phases * DEG).sin()).sin()
            }
        }
    }

    /// The first instant strictly after `t` at which the waveform (or its slope) is discontinuous: PULSE corners
    /// `td + k per + {0, tr, tr + pw, tr + pw + tf}`, the start `td` of a delayed SIN, the EXP delays and the PWL
    /// points (with their repetitions). A transient integrator must land on these instants instead of stepping
    /// across them.
    pub fn next_breakpoint(&self, t: f64) -> Option<f64> {
        let earliest = |it: &mut dyn Iterator<Item = f64>| {
            it.filter(|&c| c > t).fold(None, |acc: Option<f64>, c| {
                Some(acc.map_or(c, |a| a.min(c)))
            })
        };
        match self {
            Self::Dc(_) | Self::Sffm { .. } => None,
            &Self::Pulse {
                td,
                tr,
                tf,
                pw,
                per,
                ..
            } => {
                let (tr, tf, pw) = (tr.max(0.0), tf.max(0.0), pw.max(0.0));
                let corners = [0.0, tr, tr + pw, tr + pw + tf];
                let first_after = |base: f64, limit: f64| {
                    earliest(&mut corners.iter().filter(|&&c| c < limit).map(|c| base + c))
                };
                if per > 0.0 && per.is_finite() {
                    // `evaluate_at` folds time into one period: the corners of period k, then period k + 1
                    let k = if t > td {
                        ((t - td) / per).floor()
                    } else {
                        0.0
                    };
                    if !k.is_finite() || k > 1e15 {
                        return None;
                    }
                    let base = td + k * per;
                    first_after(base, per).or_else(|| first_after(base + per, per))
                } else {
                    first_after(td, f64::INFINITY)
                }
            }
            &Self::Sin { td, .. } => (td > t).then_some(td),
            &Self::Exp { td1, td2, .. } => earliest(&mut [td1, td2].into_iter()),
            Self::Pwl {
                points,
                repeat,
                delay,
                ..
            } => {
                let tau = t - delay;
                let &(t_last, _) = points.last()?;
                let k = points.partition_point(|p| p.0 <= tau);
                if k < points.len() {
                    return Some(points[k].0 + delay);
                }
                let r = (*repeat).filter(|r| *r < t_last)?;
                let period = t_last - r;
                let cycles = ((tau - t_last) / period).floor().max(0.0);
                if cycles > 1e15 {
                    return None;
                }
                for c in [cycles, cycles + 1.0] {
                    let base = t_last + c * period - r;
                    if let Some(bp) = earliest(
                        &mut points
                            .iter()
                            .filter(|p| p.0 > r)
                            .map(|p| base + p.0 + delay),
                    ) {
                        return Some(bp);
                    }
                }
                None
            }
        }
    }
}

/// SPICE `.model ... D(...)` parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiodeModelParams {
    pub name: String,
    pub is: f64,
    pub n: f64,
    pub rs: f64,
    pub cjo: f64,
    pub vj: f64,
    pub m: f64,
    pub tt: f64,
    /// Band gap for the `IS(T)` law, eV (default 1.11).
    #[serde(default)]
    pub eg: Option<f64>,
    /// Saturation-current temperature exponent (default 3).
    #[serde(default)]
    pub xti: Option<f64>,
    /// Temperature at which the parameters were measured, °C (default: `.options tnom`, else 27).
    #[serde(default)]
    pub tnom: Option<f64>,
}

impl Default for DiodeModelParams {
    fn default() -> Self {
        Self {
            name: "D".to_string(),
            is: 1e-14,
            n: 1.0,
            rs: 0.0,
            cjo: 0.0,
            vj: 1.0,
            m: 0.5,
            tt: 0.0,
            eg: None,
            xti: None,
            tnom: None,
        }
    }
}

/// Polarity of a MOSFET device (`NMOS` or `PMOS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosfetType {
    Nmos,
    Pmos,
}

impl MosfetType {
    #[inline]
    pub fn sign(self) -> f64 {
        match self {
            Self::Nmos => 1.0,
            Self::Pmos => -1.0,
        }
    }
}

/// SPICE `.model ... NMOS/PMOS(...)` parameters (supporting Level-1 Shichman-Hodges & EKV).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MosfetModelParams {
    pub name: String,
    pub mos_type: MosfetType,
    pub level: u32,
    pub vto: f64,
    pub kp: f64,
    pub gamma: f64,
    pub phi: f64,
    pub lambda: f64,
    pub cgso: f64,
    pub cgdo: f64,
    pub cgbo: f64,
    pub w_default: f64,
    pub l_default: f64,
    /// Temperature at which the parameters were measured, °C (default: `.options tnom`, else 27).
    #[serde(default)]
    pub tnom: Option<f64>,
}

impl Default for MosfetModelParams {
    fn default() -> Self {
        Self {
            name: "NMOS".to_string(),
            mos_type: MosfetType::Nmos,
            level: 1,
            vto: 0.0,
            kp: 2e-5,
            gamma: 0.0,
            phi: 0.6,
            lambda: 0.0,
            cgso: 0.0,
            cgdo: 0.0,
            cgbo: 0.0,
            w_default: 100e-6,
            l_default: 100e-6,
            tnom: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResistorInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub resistance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapacitorInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub capacitance: f64,
    pub ic: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InductorInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub inductance: f64,
    pub ic: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VccsInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub nc_pos: usize,
    pub nc_neg: usize,
    pub gm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoltageSourceInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub waveform: SourceWaveform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurrentSourceInstance {
    pub name: String,
    pub n_pos: usize,
    pub n_neg: usize,
    pub waveform: SourceWaveform,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiodeInstance {
    pub name: String,
    pub n_anode: usize,
    pub n_cathode: usize,
    pub model: String,
    pub area: f64,
    /// Instance temperature `temp=`, °C (default: the circuit temperature).
    #[serde(default)]
    pub temp: Option<f64>,
    /// Offset `dtemp=` from the circuit temperature, K.
    #[serde(default)]
    pub dtemp: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MosfetInstance {
    pub name: String,
    pub n_drain: usize,
    pub n_gate: usize,
    pub n_source: usize,
    pub n_bulk: usize,
    pub model: String,
    pub w: Option<f64>,
    pub l: Option<f64>,
    /// Instance multiplier `m=` (parallel copies): the effective width is `m * W`.
    #[serde(default = "unit_multiplier")]
    pub m: f64,
    /// Instance temperature `temp=`, °C (default: the circuit temperature).
    #[serde(default)]
    pub temp: Option<f64>,
    /// Offset `dtemp=` from the circuit temperature, K.
    #[serde(default)]
    pub dtemp: Option<f64>,
}

fn unit_multiplier() -> f64 {
    1.0
}

/// One `.dc` sweep: an independent source, a resistor (its value in ohm) or `temp` (°C).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DcSweep {
    pub name: String,
    pub start: f64,
    pub stop: f64,
    pub step: f64,
}

/// Simulation analysis command parsed from `.op`, `.dc`, or `.tran`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnalysisCmd {
    /// DC Operating Point (`.op`).
    Op,
    /// DC Sweep (`.dc <source> <start> <stop> <step>`).
    Dc {
        source_name: String,
        start: f64,
        stop: f64,
        step: f64,
        /// Second sweep of a nested `.dc`: the outer loop (the first sweep runs fastest).
        #[serde(default)]
        outer: Option<DcSweep>,
    },
    /// Transient Analysis (`.tran <tstep> <tstop> [<tstart> [<tmax>]] [uic]`).
    Tran {
        tstep: f64,
        tstop: f64,
        tstart: f64,
        tmax: Option<f64>,
        /// `uic`: skip the operating point and start from the element / `.ic` initial conditions.
        #[serde(default)]
        uic: bool,
    },
}

/// Complete parsed SPICE circuit netlist representation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Netlist {
    pub title: String,
    /// Node name table where index `0` is always Ground (`"0"`).
    /// Non-ground circuit nodes have 1-based node IDs `1..node_names.len()`,
    /// mapping to MNA matrix indices `0..(node_names.len() - 1)`.
    pub node_names: Vec<String>,
    pub node_map: FxHashMap<String, usize>,
    pub params: FxHashMap<String, f64>,
    pub diode_models: FxHashMap<String, DiodeModelParams>,
    pub mosfet_models: FxHashMap<String, MosfetModelParams>,
    pub resistors: Vec<ResistorInstance>,
    pub capacitors: Vec<CapacitorInstance>,
    pub inductors: Vec<InductorInstance>,
    pub vccs: Vec<VccsInstance>,
    pub v_sources: Vec<VoltageSourceInstance>,
    pub i_sources: Vec<CurrentSourceInstance>,
    pub diodes: Vec<DiodeInstance>,
    pub mosfets: Vec<MosfetInstance>,
    pub initial_conditions: FxHashMap<usize, f64>,
    /// `.nodeset` initial guesses of the operating point (node -> volts).
    #[serde(default)]
    pub nodesets: FxHashMap<usize, f64>,
    /// `.options gmin=`: conductance across every junction (`None` = 1e-12 S).
    #[serde(default)]
    pub gmin: Option<f64>,
    pub analyses: Vec<AnalysisCmd>,
    /// Circuit temperature, °C (`.temp`, `.options temp=`; `None` = 27 °C).
    #[serde(default)]
    pub temp: Option<f64>,
    /// Nominal temperature of the model parameters, °C (`.options tnom=`; `None` = 27 °C).
    #[serde(default)]
    pub tnom: Option<f64>,
    /// `tc1/tc2` of the R / C / L elements, kept to re-evaluate their values at another temperature.
    #[serde(default)]
    pub linear_tc: Vec<LinearTc>,
    /// Everything the reader ignored, approximated or re-interpreted (never changes the parsed circuit).
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl Netlist {
    /// Creates an empty [`Netlist`] with node `0` pre-registered as Ground.
    pub fn new(title: impl Into<String>) -> Self {
        let mut node_map = FxHashMap::default();
        node_map.insert("0".to_string(), 0);
        node_map.insert("gnd".to_string(), 0);
        Self {
            title: title.into(),
            node_names: vec!["0".to_string()],
            node_map,
            ..Default::default()
        }
    }

    /// Interns a node name and returns its 0-based ID (`0` = ground, `1..=N` = active nodes).
    pub fn intern_node(&mut self, raw_name: &str) -> usize {
        let key = raw_name.trim().to_ascii_lowercase();
        if key == "0" || key == "gnd" {
            return 0;
        }
        if let Some(&id) = self.node_map.get(&key) {
            return id;
        }
        let id = self.node_names.len();
        self.node_names.push(raw_name.trim().to_string());
        self.node_map.insert(key, id);
        id
    }

    /// Looks up a node's 1-based ID by name (`0` for ground).
    pub fn find_node(&self, name: &str) -> Option<usize> {
        self.node_map
            .get(&name.trim().to_ascii_lowercase())
            .copied()
    }

    /// Returns the number of non-ground voltage nodes in the circuit.
    #[inline]
    pub fn num_nodes(&self) -> usize {
        self.node_names.len().saturating_sub(1)
    }

    /// Returns the total MNA matrix dimension (`num_nodes + v_sources.len() + inductors.len()`).
    #[inline]
    pub fn mna_dimension(&self) -> usize {
        self.num_nodes() + self.v_sources.len() + self.inductors.len()
    }

    /// Fails if a source amplitude / frequency or an analysis bound is infinite or NaN.
    ///
    /// A literal such as `1e999` parses to `+inf` (it overflows `f64`). For an independent source amplitude
    /// (`DC`, `PULSE v1 v2`, `SIN vo va freq`) or a `.dc` / `.tran` bound no finite answer exists (the former
    /// behaviour was NaN output rows or an endless run with exit status 0), while ngspice itself fails on
    /// these. Device values (`R`, `C`, `L`, `G`, ...) and the timing fields of `PULSE` / `SIN` are NOT
    /// checked: an infinite resistance is a valid open circuit (both tools agree), and a non-finite
    /// RESULT is caught by the solver (`SimError::NonFiniteSolution`).
    pub fn validate_finite(&self) -> SimResult<()> {
        fn bad(fields: &[(&'static str, f64)]) -> Option<(&'static str, f64)> {
            fields.iter().copied().find(|(_, v)| !v.is_finite())
        }
        // the derived `.op` value last: a non-finite argument is reported by its own name first
        fn waveform_fields(w: &SourceWaveform) -> Vec<(&'static str, f64)> {
            match w {
                SourceWaveform::Dc(v) => vec![("dc value", *v)],
                SourceWaveform::Pulse { dc_val, v1, v2, .. } => vec![
                    ("PULSE v1", *v1),
                    ("PULSE v2", *v2),
                    ("PULSE dc value", *dc_val),
                ],
                SourceWaveform::Sin {
                    dc_val,
                    vo,
                    va,
                    freq,
                    ..
                } => {
                    let mut f = vec![("SIN vo", *vo), ("SIN va", *va)];
                    // a NaN frequency is the 1/tstop default, resolved by the transient
                    if !freq.is_nan() {
                        f.push(("SIN frequency", *freq));
                    }
                    f.push(("SIN dc value", *dc_val));
                    f
                }
                SourceWaveform::Exp { dc_val, v1, v2, .. } => {
                    vec![("EXP v1", *v1), ("EXP v2", *v2), ("EXP dc value", *dc_val)]
                }
                SourceWaveform::Pwl { dc_val, points, .. } => {
                    let mut f: Vec<(&'static str, f64)> =
                        points.iter().map(|p| ("PWL value", p.1)).collect();
                    f.extend(points.iter().map(|p| ("PWL time", p.0)));
                    f.push(("PWL dc value", *dc_val));
                    f
                }
                SourceWaveform::Sffm { dc_val, vo, va, .. } => vec![
                    ("SFFM vo", *vo),
                    ("SFFM va", *va),
                    ("SFFM dc value", *dc_val),
                ],
            }
        }
        let fail = |name: &str, field: &str, v: f64| {
            Err(SimError::TopologyError(format!(
                "non-finite value ({v}) for {field} of `{name}`: a numeric literal that overflows f64 (e.g. 1e999) is not a valid source amplitude or analysis bound"
            )))
        };
        for src in &self.v_sources {
            if let Some((f, v)) = bad(&waveform_fields(&src.waveform)) {
                return fail(&src.name, f, v);
            }
        }
        for src in &self.i_sources {
            if let Some((f, v)) = bad(&waveform_fields(&src.waveform)) {
                return fail(&src.name, f, v);
            }
        }
        for a in &self.analyses {
            match a {
                AnalysisCmd::Op => {}
                AnalysisCmd::Dc {
                    start,
                    stop,
                    step,
                    outer,
                    ..
                } => {
                    if let Some((f, v)) =
                        bad(&[("start", *start), ("stop", *stop), ("step", *step)])
                    {
                        return fail(".dc", f, v);
                    }
                    if let Some(o) = outer {
                        if let Some((f, v)) =
                            bad(&[("start2", o.start), ("stop2", o.stop), ("step2", o.step)])
                        {
                            return fail(".dc", f, v);
                        }
                    }
                }
                AnalysisCmd::Tran { tstep, tstop, .. } => {
                    if let Some((f, v)) = bad(&[("tstep", *tstep), ("tstop", *tstop)]) {
                        return fail(".tran", f, v);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Fluent Rust builder for programmatic circuit construction.
pub struct CircuitBuilder {
    netlist: Netlist,
}

impl CircuitBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            netlist: Netlist::new(title),
        }
    }

    pub fn add_resistor(mut self, name: &str, n_pos: &str, n_neg: &str, r: f64) -> Self {
        let p = self.netlist.intern_node(n_pos);
        let n = self.netlist.intern_node(n_neg);
        self.netlist.resistors.push(ResistorInstance {
            name: name.to_string(),
            n_pos: p,
            n_neg: n,
            resistance: r,
        });
        self
    }

    pub fn add_capacitor(mut self, name: &str, n_pos: &str, n_neg: &str, c: f64) -> Self {
        let p = self.netlist.intern_node(n_pos);
        let n = self.netlist.intern_node(n_neg);
        self.netlist.capacitors.push(CapacitorInstance {
            name: name.to_string(),
            n_pos: p,
            n_neg: n,
            capacitance: c,
            ic: None,
        });
        self
    }

    pub fn add_inductor(mut self, name: &str, n_pos: &str, n_neg: &str, l: f64) -> Self {
        let p = self.netlist.intern_node(n_pos);
        let n = self.netlist.intern_node(n_neg);
        self.netlist.inductors.push(InductorInstance {
            name: name.to_string(),
            n_pos: p,
            n_neg: n,
            inductance: l,
            ic: None,
        });
        self
    }

    pub fn add_vsource_dc(mut self, name: &str, n_pos: &str, n_neg: &str, v: f64) -> Self {
        let p = self.netlist.intern_node(n_pos);
        let n = self.netlist.intern_node(n_neg);
        self.netlist.v_sources.push(VoltageSourceInstance {
            name: name.to_string(),
            n_pos: p,
            n_neg: n,
            waveform: SourceWaveform::Dc(v),
        });
        self
    }

    pub fn add_isource_dc(mut self, name: &str, n_pos: &str, n_neg: &str, i: f64) -> Self {
        let p = self.netlist.intern_node(n_pos);
        let n = self.netlist.intern_node(n_neg);
        self.netlist.i_sources.push(CurrentSourceInstance {
            name: name.to_string(),
            n_pos: p,
            n_neg: n,
            waveform: SourceWaveform::Dc(i),
        });
        self
    }

    pub fn add_diode(mut self, name: &str, anode: &str, cathode: &str, model: &str) -> Self {
        let a = self.netlist.intern_node(anode);
        let c = self.netlist.intern_node(cathode);
        let m_key = model.to_ascii_lowercase();
        self.netlist
            .diode_models
            .entry(m_key.clone())
            .or_insert_with(|| DiodeModelParams {
                name: model.to_string(),
                ..Default::default()
            });
        self.netlist.diodes.push(DiodeInstance {
            name: name.to_string(),
            n_anode: a,
            n_cathode: c,
            model: m_key,
            area: 1.0,
            temp: None,
            dtemp: None,
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_mosfet(
        mut self,
        name: &str,
        d: &str,
        g: &str,
        s: &str,
        b: &str,
        model: &str,
        w: f64,
        l: f64,
    ) -> Self {
        let nd = self.netlist.intern_node(d);
        let ng = self.netlist.intern_node(g);
        let ns = self.netlist.intern_node(s);
        let nb = self.netlist.intern_node(b);
        let m_key = model.to_ascii_lowercase();
        self.netlist
            .mosfet_models
            .entry(m_key.clone())
            .or_insert_with(|| MosfetModelParams {
                name: model.to_string(),
                mos_type: if m_key.starts_with('p') {
                    MosfetType::Pmos
                } else {
                    MosfetType::Nmos
                },
                vto: if m_key.starts_with('p') { -0.7 } else { 0.7 },
                kp: 120e-6,
                lambda: 0.02,
                ..Default::default()
            });
        self.netlist.mosfets.push(MosfetInstance {
            name: name.to_string(),
            n_drain: nd,
            n_gate: ng,
            n_source: ns,
            n_bulk: nb,
            model: m_key,
            w: Some(w),
            l: Some(l),
            m: 1.0,
            temp: None,
            dtemp: None,
        });
        self
    }

    pub fn build(self) -> Netlist {
        self.netlist
    }
}

#[derive(Clone, Debug)]
struct SubcktDef {
    ports: Vec<String>,
    /// `PARAMS:` defaults of the header, `(name, expression)`.
    params: Vec<(String, String)>,
    lines: Vec<(usize, String)>,
}

/// Index of the first token of `toks` that starts a parameter list: `params:` or a `name=value` / `name = value`
/// assignment.
fn param_list_start(toks: &[&str]) -> usize {
    toks.iter()
        .position(|t| {
            let l = t.to_ascii_lowercase();
            l.starts_with("params:") || t.contains('=')
        })
        .unwrap_or(toks.len())
}

/// The assignments of a `PARAMS:` list (the keyword itself is optional).
fn parse_param_list(text: &str) -> Result<Vec<(String, String)>, String> {
    let lower = text.to_ascii_lowercase();
    let body = match lower.find("params:") {
        Some(i) => format!("{} {}", &text[..i], &text[i + 7..]),
        None => text.to_string(),
    };
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    crate::expr::split_assignments(&body)
}

/// `.subckt <name> <ports...> [PARAMS:] [a=1 b={...}]`
fn parse_subckt_header(line: &str) -> Result<(String, SubcktDef), String> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.len() < 2 {
        return Err(".subckt requires a name".to_string());
    }
    let rest = &toks[2..];
    let k = param_list_start(rest);
    // a `name = value` pair written with blanks: the name is a positional token in front of a lone `=`
    let k = if k < rest.len() && rest[k].starts_with('=') && k > 0 {
        k - 1
    } else {
        k
    };
    let ports = rest[..k].iter().map(|s| (*s).to_string()).collect();
    let params = parse_param_list(&rest[k..].join(" "))?;
    Ok((
        toks[1].to_ascii_lowercase(),
        SubcktDef {
            ports,
            params,
            lines: Vec::new(),
        },
    ))
}

/// `.func <name>(<args>) [=] <body>`
fn parse_func_card(rest: &str) -> Result<(String, crate::expr::FuncDef), String> {
    let rest = rest.trim();
    let open = rest.find('(').ok_or("`.func` needs `name(args)`")?;
    let close = rest[open..]
        .find(')')
        .ok_or("`.func` argument list is not closed")?
        + open;
    let name = rest[..open].trim().to_ascii_lowercase();
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(format!("`{name}` is not a function name"));
    }
    let params: Vec<String> = rest[open + 1..close]
        .split(',')
        .map(|a| a.trim().to_ascii_lowercase())
        .filter(|a| !a.is_empty())
        .collect();
    let body = rest[close + 1..].trim().trim_start_matches('=').trim();
    if body.is_empty() {
        return Err(format!("`.func {name}` has no body"));
    }
    Ok((
        name,
        crate::expr::FuncDef {
            params,
            body: crate::expr::unwrap_delimiters(body).to_string(),
        },
    ))
}

/// Mutable state shared by the second parsing pass (diagnostics bookkeeping only; never alters the circuit).
#[derive(Default)]
struct ParseState {
    /// Hashes of the lower-cased element names seen so far (duplicate detection without allocation).
    seen_names: FxHashSet<u64>,
    /// Element `ic=` fields (line, description) whose severity depends on `uic`.
    pending_elem_ic: Vec<(usize, String)>,
    /// Line of the first `.tran ... uic` card.
    uic_line: Option<usize>,
    /// R / C / L temperature coefficients, applied once the circuit temperature is known (a `.temp` card may
    /// follow the element).
    pending_tc: Vec<PendingTc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum LinearKind {
    R,
    C,
    L,
}

/// Polynomial temperature coefficients of one R / C / L: `value(T) = nominal (1 + tc1 dT + tc2 dT^2)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinearTc {
    pub kind: LinearKind,
    pub index: usize,
    /// Value at the nominal temperature.
    pub nominal: f64,
    pub tc1: f64,
    pub tc2: f64,
    /// Instance `temp=` (°C), overriding the circuit temperature.
    pub temp: Option<f64>,
    pub dtemp: f64,
}

struct PendingTc {
    kind: LinearKind,
    index: usize,
    tc1: f64,
    tc2: f64,
    temp: Option<f64>,
    dtemp: f64,
    line: usize,
}

impl Netlist {
    fn linear_value_mut(&mut self, kind: LinearKind, index: usize) -> (&str, &mut f64) {
        match kind {
            LinearKind::R => {
                let r = &mut self.resistors[index];
                (&r.name, &mut r.resistance)
            }
            LinearKind::C => {
                let c = &mut self.capacitors[index];
                (&c.name, &mut c.capacitance)
            }
            LinearKind::L => {
                let l = &mut self.inductors[index];
                (&l.name, &mut l.inductance)
            }
        }
    }

    /// Sets the circuit temperature (°C) and re-evaluates the R / C / L values that carry `tc1/tc2`. The device
    /// models read the temperature when the netlist is compiled.
    pub fn set_temperature(&mut self, t: f64) -> SimResult<()> {
        check_temperature(t, "temperature", 0)?;
        self.temp = Some(t);
        let tnom = self.tnom.unwrap_or(crate::temperature::DEFAULT_TEMP_C);
        for k in 0..self.linear_tc.len() {
            let tc = self.linear_tc[k].clone();
            let dt = tc.temp.unwrap_or(t) + tc.dtemp - tnom;
            let f = crate::temperature::tc_factor(tc.tc1, tc.tc2, dt);
            let (name, value) = self.linear_value_mut(tc.kind, tc.index);
            if f <= 0.0 {
                return Err(SimError::TopologyError(format!(
                    "{name}: the temperature factor 1 + tc1 dT + tc2 dT^2 = {f} at {t} °C is not positive"
                )));
            }
            *value = tc.nominal * f;
        }
        Ok(())
    }
}

impl PendingTc {
    /// Records the `tc1/tc2/temp/dtemp` fields of the element just pushed (nothing when it has none).
    fn record(
        state: &mut ParseState,
        kind: LinearKind,
        index: usize,
        kv: &FxHashMap<String, f64>,
        line: usize,
    ) {
        let get = |k: &str| kv.get(k).copied();
        let (tc1, tc2) = (get("tc1").unwrap_or(0.0), get("tc2").unwrap_or(0.0));
        if tc1 == 0.0 && tc2 == 0.0 {
            return;
        }
        state.pending_tc.push(PendingTc {
            kind,
            index,
            tc1,
            tc2,
            temp: get("temp"),
            dtemp: get("dtemp").unwrap_or(0.0),
            line,
        });
    }
}

/// Lowest temperature a deck may ask for, °C.
const ABSOLUTE_ZERO_C: f64 = -crate::temperature::KELVIN;

fn check_temperature(t: f64, what: &str, line: usize) -> SimResult<f64> {
    if t.is_finite() && t > ABSOLUTE_ZERO_C {
        Ok(t)
    } else {
        Err(SimError::ParseError {
            line,
            message: format!(
                "{what} = {t} °C is at or below absolute zero ({ABSOLUTE_ZERO_C} °C): no physical operating point exists"
            ),
        })
    }
}

fn name_hash(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b.to_ascii_lowercase());
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Shortens a source line for use inside a diagnostic.
fn clip(text: &str) -> String {
    const MAX: usize = 60;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut end = MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// Parses a complete SPICE netlist string into a [`Netlist`] AST with default [`ResourceLimits`].
///
/// Constructs the reader ignores, approximates or re-interprets are recorded in [`Netlist::diagnostics`]
/// (they never change the parsed circuit and never fail the parse).
pub fn parse_netlist(src: &str) -> SimResult<Netlist> {
    parse_netlist_with_limits(src, &ResourceLimits::default())
}

/// Like [`parse_netlist`] with explicit hierarchy-expansion budgets (`max_subckt_depth`,
/// `max_expanded_cards`, `max_expanded_bytes`).
pub fn parse_netlist_with_limits(src: &str, limits: &ResourceLimits) -> SimResult<Netlist> {
    parse_source(src, None, limits)
}

/// Reads and parses the deck file at `path`; `.include` / `.lib` names resolve against its directory.
pub fn parse_netlist_file(path: &std::path::Path, limits: &ResourceLimits) -> SimResult<Netlist> {
    let bytes = std::fs::read(path).map_err(|e| SimError::ParseError {
        line: 0,
        message: format!("cannot read `{}`: {e}", path.display()),
    })?;
    let src = crate::include::decode_deck(&bytes)?;
    parse_source(&src, Some(path), limits)
}

fn parse_source(
    src: &str,
    deck: Option<&std::path::Path>,
    limits: &ResourceLimits,
) -> SimResult<Netlist> {
    let physical = crate::include::expand(src, deck, limits)?;
    let mut sink = DiagSink::new();
    let mut logical_lines: Vec<(usize, String)> = Vec::new();
    let mut title = String::from("Untitled SPICE Deck");
    let mut in_control_block = false;
    let mut control_start = 0usize;
    let mut control_lines = 0usize;

    for (idx, phys) in physical.iter().enumerate() {
        let line_no = phys.line;
        let trimmed = phys.text.trim();
        // SPICE: physical line 1 is the title, whatever it holds (blank, a comment, something card-like)
        if idx == 0 {
            if !trimmed.is_empty() {
                title = trimmed.to_string();
            }
            if trimmed.starts_with('.') {
                sink.note(
                    line_no,
                    code::TITLE_LINE,
                    format!(
                        "line 1 `{}` is the title (SPICE convention) and is not simulated",
                        clip(trimmed)
                    ),
                );
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('*') || trimmed.starts_with(';') {
            continue;
        }
        let lower_head = trimmed.to_ascii_lowercase();
        if lower_head.starts_with(".control") {
            in_control_block = true;
            control_start = line_no;
            control_lines = 0;
            continue;
        }
        if in_control_block && !lower_head.starts_with(".endc") {
            // output and run commands are harmless to skip; anything that changes the circuit or the analyses
            // would make the results differ from what the deck asks for
            let cmd = lower_head.split_whitespace().next().unwrap_or("");
            const HARMLESS: &[&str] = &[
                "run", "write", "wrdata", "print", "plot", "echo", "quit", "exit", "display",
                "setplot", "rusage", "listing", "show", "showmod", "version", "destroy", "*",
            ];
            if !cmd.starts_with('*') && !HARMLESS.contains(&cmd) {
                sink.fatal(
                    line_no,
                    format!("`.control` command `{cmd}` is not supported (control scripts are not executed)"),
                );
            }
        }
        if lower_head.starts_with(".endc") {
            if in_control_block {
                sink.warn(
                    control_start,
                    code::IGNORED_ANALYSIS,
                    format!(
                        ".control block ({control_lines} lines) is ignored: scripted commands (alter, run, let, ...) are not executed"
                    ),
                );
            } else {
                sink.warn(
                    line_no,
                    code::MALFORMED_INPUT,
                    "`.endc` without a matching `.control` was ignored",
                );
            }
            in_control_block = false;
            continue;
        }
        if in_control_block {
            control_lines += 1;
            continue;
        }
        // Strip inline comments: `;` anywhere, `$` at the start of a token (after a blank or a comma), so node
        // names such as `n$1` survive
        let no_semicolon = trimmed.split(';').next().unwrap_or("");
        let dollar = no_semicolon
            .char_indices()
            .find(|&(i, c)| {
                c == '$'
                    && (i == 0
                        || no_semicolon[..i]
                            .chars()
                            .next_back()
                            .is_some_and(|p| p.is_whitespace() || p == ','))
            })
            .map_or(no_semicolon.len(), |(i, _)| i);
        let clean = no_semicolon[..dollar].trim();
        if clean.is_empty() {
            continue;
        }
        if clean.contains('\u{FFFD}') {
            return Err(SimError::ParseError {
                line: line_no,
                message: "the card contains bytes that are not valid UTF-8 (only the title and comments may)"
                    .to_string(),
            });
        }
        if let Some(cont) = clean.strip_prefix('+') {
            if let Some(last) = logical_lines.last_mut() {
                last.1.push(' ');
                last.1.push_str(cont.trim());
            } else {
                sink.warn(
                    line_no,
                    code::MALFORMED_INPUT,
                    "continuation line `+...` has no preceding card and was ignored",
                );
            }
        } else {
            logical_lines.push((line_no, clean.to_string()));
        }
    }
    if in_control_block {
        sink.warn(
            control_start,
            code::IGNORED_ANALYSIS,
            format!(
                ".control block ({control_lines} lines, never closed by .endc) is ignored: scripted commands are not executed"
            ),
        );
    }

    let mut netlist = Netlist::new(title);
    let mut subckts: FxHashMap<String, SubcktDef> = FxHashMap::default();
    // open `.subckt` definitions (a definition inside another one is hoisted to the circuit-wide namespace)
    let mut open_subckts: Vec<(usize, String, SubcktDef)> = Vec::new();
    let mut top_lines: Vec<(usize, String)> = Vec::new();
    let mut param_defs: Vec<(String, String)> = Vec::new();
    let mut param_lines: FxHashMap<String, usize> = FxHashMap::default();
    let mut funcs: FxHashMap<String, crate::expr::FuncDef> = FxHashMap::default();
    let mut model_lines: Vec<(usize, String)> = Vec::new();
    let mut global_nodes: FxHashSet<String> = FxHashSet::default();

    // First pass: collect `.param`, `.func`, `.model`, and `.subckt` definitions
    for (line_no, line) in logical_lines {
        let lower = line.to_ascii_lowercase();
        let is_card = |kw: &str| {
            lower.starts_with(kw)
                && lower[kw.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| c.is_whitespace())
        };
        if is_card(".subckt") {
            let (name, def) =
                parse_subckt_header(&line).map_err(|message| SimError::ParseError {
                    line: line_no,
                    message,
                })?;
            open_subckts.push((line_no, name, def));
            continue;
        }
        if is_card(".ends") {
            if let Some((_, name, def)) = open_subckts.pop() {
                subckts.insert(name, def);
            } else {
                sink.warn(
                    line_no,
                    code::MALFORMED_INPUT,
                    "`.ends` without a matching `.subckt` was ignored",
                );
            }
            continue;
        }
        // `.model` cards are circuit-wide, also when written inside a `.subckt` body
        if is_card(".model") {
            model_lines.push((line_no, line));
            continue;
        }
        if let Some((_, _, sub)) = open_subckts.last_mut() {
            sub.lines.push((line_no, line));
            continue;
        }
        if is_card(".global") {
            for n in line.split_whitespace().skip(1) {
                global_nodes.insert(n.to_ascii_lowercase());
            }
            continue;
        }
        if is_card(".param") {
            match crate::expr::split_assignments(&line[6..]) {
                Ok(defs) => {
                    for (k, v) in defs {
                        param_lines.insert(k.clone(), line_no);
                        param_defs.push((k, v));
                    }
                }
                Err(e) => sink.fatal(line_no, format!("`.param`: {e}")),
            }
        } else if is_card(".func") {
            match parse_func_card(&line[5..]) {
                Ok((name, def)) => {
                    funcs.insert(name, def);
                }
                Err(e) => sink.fatal(line_no, format!("`.func`: {e}")),
            }
        } else {
            top_lines.push((line_no, line));
        }
    }
    if let Some((start, name, _)) = open_subckts.first() {
        return Err(SimError::ParseError {
            line: *start,
            message: format!("`.subckt {name}` has no matching `.ends`"),
        });
    }
    for (name, reason) in crate::expr::resolve_params(&param_defs, &mut netlist.params, &funcs) {
        sink.fatal(
            param_lines.get(&name).copied().unwrap_or(0),
            format!("`.param {name}` cannot be evaluated: {reason}"),
        );
    }
    for (line_no, line) in model_lines {
        let scope = crate::expr::Scope {
            globals: &netlist.params,
            locals: None,
            funcs: &funcs,
        };
        let text = crate::expr::substitute(&line, scope).map_err(|e| SimError::ParseError {
            line: line_no,
            message: format!("{e} in `{}`", clip(&line)),
        })?;
        parse_model_card(line_no, &text, &mut netlist, &mut sink)?;
    }

    // Expand subcircuits (`X...`) with an explicit stack: bounded by `limits`, never by the call stack
    let expanded_lines = expand_subcircuits(
        &top_lines,
        &subckts,
        &netlist.params,
        &funcs,
        &global_nodes,
        limits,
    )?;

    // Second pass: parse all elements and analysis cards
    let mut state = ParseState::default();
    state.seen_names.reserve(expanded_lines.len());
    for (line_no, line) in expanded_lines {
        parse_statement(line_no, &line, &mut netlist, &mut sink, &mut state)?;
    }

    let circuit_temp = netlist.temp.unwrap_or(crate::temperature::DEFAULT_TEMP_C);
    let tnom = netlist.tnom.unwrap_or(crate::temperature::DEFAULT_TEMP_C);
    for p in std::mem::take(&mut state.pending_tc) {
        let t = check_temperature(
            p.temp.unwrap_or(circuit_temp) + p.dtemp,
            "device temperature",
            p.line,
        )?;
        let f = crate::temperature::tc_factor(p.tc1, p.tc2, t - tnom);
        let (_, nominal) = netlist.linear_value_mut(p.kind, p.index);
        let nominal = *nominal;
        netlist.linear_tc.push(LinearTc {
            kind: p.kind,
            index: p.index,
            nominal,
            tc1: p.tc1,
            tc2: p.tc2,
            temp: p.temp,
            dtemp: p.dtemp,
        });
        let (name, value) = netlist.linear_value_mut(p.kind, p.index);
        if f <= 0.0 {
            sink.warn(
                p.line,
                code::MALFORMED_INPUT,
                format!("{name}: the temperature factor 1 + tc1 dT + tc2 dT^2 = {f} at {t} °C is not positive; the value is used as written"),
            );
        } else {
            *value *= f;
        }
    }
    if state.uic_line.is_none() {
        for (ln, what) in std::mem::take(&mut state.pending_elem_ic) {
            sink.note(
                ln,
                code::IGNORED_IC,
                format!("{what} is not applied: element initial conditions only take effect with `.tran ... uic` (as in SPICE)"),
            );
        }
    }

    if netlist.analyses.is_empty() {
        netlist.analyses.push(AnalysisCmd::Op);
        if sink.has(code::IGNORED_ANALYSIS) {
            sink.warn(
                0,
                code::IGNORED_ANALYSIS,
                "no supported analysis card (.op/.dc/.tran) remains: a DC .op is run instead",
            );
        } else {
            sink.note(
                0,
                code::IGNORED_ANALYSIS,
                "no .op/.dc/.tran card found: a DC .op is run",
            );
        }
    }

    if let Some((line, message)) = sink.take_fatal() {
        return Err(SimError::ParseError { line, message });
    }
    netlist.diagnostics = sink.finish();
    netlist.validate_finite()?;

    Ok(netlist)
}

/// One level of the explicit hierarchy stack used by [`expand_subcircuits`].
struct ExpandFrame<'a> {
    /// Lower-case subcircuit name (empty for the top level).
    name: &'a str,
    lines: &'a [(usize, String)],
    next: usize,
    prefix: String,
    port_map: FxHashMap<String, String>,
    /// Parameters of this instance and of the instances around it (looked up before the circuit-wide ones).
    locals: FxHashMap<String, f64>,
}

fn expansion_limit(
    resource: &'static str,
    requested: String,
    limit: String,
    hint: &str,
) -> SimError {
    SimError::ResourceLimit {
        resource,
        requested,
        limit,
        hint: hint.to_string(),
    }
}

/// Flattens `X` instances depth-first with an explicit stack (same card order as plain recursion).
///
/// * every `{...}` / `'...'` expression of a flattened card is evaluated in the scope of its instance (the
///   instance's `name=value` overrides, the `PARAMS:` defaults of the definition and its `.param` cards, the
///   instances around it, then the circuit-wide `.param` values) and replaced by its value;
/// * a subcircuit instantiating itself, directly or through other subcircuits, is refused with the cycle
///   spelled out;
/// * `limits.max_subckt_depth`, `limits.max_expanded_cards` (instance + element cards visited) and
///   `limits.max_expanded_bytes` (flattened text) are enforced while expanding.
fn expand_subcircuits(
    top_lines: &[(usize, String)],
    subckts: &FxHashMap<String, SubcktDef>,
    globals: &FxHashMap<String, f64>,
    funcs: &FxHashMap<String, crate::expr::FuncDef>,
    global_nodes: &FxHashSet<String>,
    limits: &ResourceLimits,
) -> SimResult<Vec<(usize, String)>> {
    let mut out: Vec<(usize, String)> = Vec::with_capacity(top_lines.len());
    let mut cards = 0usize;
    let mut bytes = 0usize;
    let mut active: FxHashSet<&str> = FxHashSet::default();
    let mut stack: Vec<ExpandFrame> = vec![ExpandFrame {
        name: "",
        lines: top_lines,
        next: 0,
        prefix: String::new(),
        port_map: FxHashMap::default(),
        locals: FxHashMap::default(),
    }];

    while let Some(frame) = stack.last_mut() {
        let lines: &[(usize, String)] = frame.lines;
        let Some((line_no, raw_line)) = lines.get(frame.next) else {
            if let Some(done) = stack.pop() {
                active.remove(done.name);
            }
            continue;
        };
        frame.next += 1;
        cards += 1;
        if cards > limits.max_expanded_cards {
            return Err(expansion_limit(
                "expanded_cards",
                format!(
                    "more than {} flattened cards (stopped at line {line_no}, depth {})",
                    limits.max_expanded_cards,
                    stack.len() - 1
                ),
                format!("{} cards (max_expanded_cards)", limits.max_expanded_cards),
                "the hierarchy flattens to more cards than the budget; reduce the fan-out or raise --max-expanded-cards",
            ));
        }
        let first_char = raw_line.as_bytes()[0].to_ascii_uppercase();
        if !frame.name.is_empty() && first_char == b'.' {
            let lower = raw_line.to_ascii_lowercase();
            // `.param` cards of a definition were evaluated when the instance was entered
            if lower.starts_with(".param") {
                continue;
            }
        }
        let scope = crate::expr::Scope {
            globals,
            locals: Some(&frame.locals),
            funcs,
        };
        let substituted;
        let line: &String = if raw_line.contains(['{', '\'']) && !is_verbatim_card(raw_line) {
            substituted =
                crate::expr::substitute(raw_line, scope).map_err(|e| SimError::ParseError {
                    line: *line_no,
                    message: format!("{e} in `{}`", clip(raw_line)),
                })?;
            &substituted
        } else {
            raw_line
        };
        if first_char == b'X' {
            let toks: Vec<&str> = line.split_whitespace().collect();
            let inst_name = toks[0];
            let rest = &toks[1..];
            let mut k = param_list_start(rest);
            if k < rest.len() && rest[k].starts_with('=') && k > 0 {
                k -= 1;
            }
            if k == 0 {
                return Err(SimError::ParseError {
                    line: *line_no,
                    message: "Invalid X subcircuit instance card: no subcircuit name".to_string(),
                });
            }
            let sub_name = rest[k - 1].to_ascii_lowercase();
            let actual_nodes = &rest[..k - 1];
            let overrides =
                parse_param_list(&rest[k..].join(" ")).map_err(|message| SimError::ParseError {
                    line: *line_no,
                    message: format!("{inst_name}: {message}"),
                })?;
            let (sub_key, sub) =
                subckts
                    .get_key_value(&sub_name)
                    .ok_or_else(|| SimError::ParseError {
                        line: *line_no,
                        message: format!("Unknown subcircuit `{sub_name}`"),
                    })?;
            if actual_nodes.len() != sub.ports.len() {
                return Err(SimError::ParseError {
                    line: *line_no,
                    message: format!(
                        "Subcircuit `{sub_name}` expects {} ports, got {}",
                        sub.ports.len(),
                        actual_nodes.len()
                    ),
                });
            }
            let new_prefix = if frame.prefix.is_empty() {
                inst_name.to_string()
            } else {
                format!("{}.{inst_name}", frame.prefix)
            };
            let mut child_map = FxHashMap::default();
            for (formal, actual) in sub.ports.iter().zip(actual_nodes.iter()) {
                let mapped_actual = map_subckt_node(actual, &frame.prefix, &frame.port_map);
                child_map.insert(formal.to_ascii_lowercase(), mapped_actual);
            }
            // `.global` nodes keep their name at every level
            for g in global_nodes {
                child_map.entry(g.clone()).or_insert_with(|| g.clone());
            }
            let locals = instance_scope(sub, &overrides, &frame.locals, globals, funcs).map_err(
                |message| SimError::ParseError {
                    line: *line_no,
                    message: format!("{inst_name} (`{sub_name}`): {message}"),
                },
            )?;

            if active.contains(sub_key.as_str()) {
                let first = stack
                    .iter()
                    .position(|f| f.name == sub_key.as_str())
                    .unwrap_or(0);
                let mut chain: Vec<&str> = stack[first..].iter().map(|f| f.name).collect();
                chain.push(sub_key.as_str());
                return Err(SimError::ParseError {
                    line: *line_no,
                    message: format!(
                        "Recursive subcircuit instantiation `{}` (instance `{inst_name}`): the expansion would never terminate",
                        chain.join(" -> ")
                    ),
                });
            }
            let child_depth = stack.len();
            if child_depth > limits.max_subckt_depth {
                return Err(expansion_limit(
                    "subckt_depth",
                    format!(
                        "instantiation depth {child_depth} (instance `{inst_name}` of `{sub_name}`, line {line_no}) exceeds the limit"
                    ),
                    format!("{} levels (max_subckt_depth)", limits.max_subckt_depth),
                    "flatten the hierarchy or raise --max-subckt-depth",
                ));
            }
            active.insert(sub_key.as_str());
            stack.push(ExpandFrame {
                name: sub_key.as_str(),
                lines: sub.lines.as_slice(),
                next: 0,
                prefix: new_prefix,
                port_map: child_map,
                locals,
            });
        } else {
            let text = if frame.prefix.is_empty() {
                line.clone()
            } else {
                rewrite_subckt_line(line, &frame.prefix, &frame.port_map)
            };
            bytes += text.len();
            if bytes > limits.max_expanded_bytes {
                return Err(expansion_limit(
                    "expanded_bytes",
                    format!(
                        "more than {} bytes of flattened netlist text (stopped at line {line_no})",
                        limits.max_expanded_bytes
                    ),
                    format!("{} bytes (max_expanded_bytes)", limits.max_expanded_bytes),
                    "flattened instance names grow with the nesting depth; reduce depth or raise --max-expanded-mib",
                ));
            }
            out.push((*line_no, text));
        }
    }
    Ok(out)
}

/// Cards whose text is not an expression context (file names and similar may contain quotes or braces).
fn is_verbatim_card(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    [
        ".include", ".inc", ".lib", ".title", ".save", ".print", ".plot", ".probe", ".control",
    ]
    .iter()
    .any(|kw| lower.starts_with(kw))
}

/// Parameter values inside one instance of `sub`: the enclosing instances' values, then the definition's
/// `PARAMS:` defaults replaced by the instance overrides (evaluated in the enclosing scope), then the `.param`
/// cards of the definition body.
fn instance_scope(
    sub: &SubcktDef,
    overrides: &[(String, String)],
    outer: &FxHashMap<String, f64>,
    globals: &FxHashMap<String, f64>,
    funcs: &FxHashMap<String, crate::expr::FuncDef>,
) -> Result<FxHashMap<String, f64>, String> {
    let has_body_params = sub
        .lines
        .iter()
        .any(|(_, l)| l.len() >= 6 && l[..6].eq_ignore_ascii_case(".param"));
    if sub.params.is_empty() && overrides.is_empty() && !has_body_params {
        return Ok(outer.clone());
    }
    let mut table = outer.clone();
    let outer_scope = crate::expr::Scope {
        globals,
        locals: Some(outer),
        funcs,
    };
    let mut given: FxHashSet<&str> = FxHashSet::default();
    for (name, expr) in overrides {
        if !sub.params.iter().any(|(p, _)| p == name) {
            return Err(format!("`{name}` is not a parameter of the subcircuit"));
        }
        let v = crate::expr::eval(expr, outer_scope).map_err(|e| format!("{name}={expr}: {e}"))?;
        table.insert(name.clone(), v);
        given.insert(name.as_str());
    }
    let defaults: Vec<(String, String)> = sub
        .params
        .iter()
        .filter(|(p, _)| !given.contains(p.as_str()))
        .cloned()
        .collect();
    let failed = crate::expr::resolve_params_in(&defaults, &mut table, Some(globals), funcs);
    if let Some((name, reason)) = failed.first() {
        return Err(format!("parameter `{name}`: {reason}"));
    }
    if has_body_params {
        let mut body_defs: Vec<(String, String)> = Vec::new();
        for (_, l) in &sub.lines {
            if l.len() >= 6 && l[..6].eq_ignore_ascii_case(".param") {
                body_defs.extend(crate::expr::split_assignments(&l[6..])?);
            }
        }
        let failed = crate::expr::resolve_params_in(&body_defs, &mut table, Some(globals), funcs);
        if let Some((name, reason)) = failed.first() {
            return Err(format!("`.param {name}`: {reason}"));
        }
    }
    Ok(table)
}

fn map_subckt_node(raw: &str, prefix: &str, port_map: &FxHashMap<String, String>) -> String {
    let key = raw.to_ascii_lowercase();
    if key == "0" || key == "gnd" {
        return "0".to_string();
    }
    if let Some(mapped) = port_map.get(&key) {
        return mapped.clone();
    }
    if prefix.is_empty() {
        raw.to_string()
    } else {
        format!("{prefix}.{raw}")
    }
}

fn rewrite_subckt_line(line: &str, prefix: &str, port_map: &FxHashMap<String, String>) -> String {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.is_empty() {
        return String::new();
    }
    let lead = toks[0].as_bytes()[0].to_ascii_uppercase();
    let num_nodes = match lead {
        b'R' | b'C' | b'L' | b'V' | b'I' | b'D' => 2,
        b'G' | b'E' | b'M' => 4,
        _ => return line.to_string(),
    };
    let mut parts = Vec::with_capacity(toks.len());
    parts.push(format!("{}_{}", toks[0], prefix));
    for (idx, tok) in toks.iter().enumerate().skip(1) {
        if idx <= num_nodes {
            parts.push(map_subckt_node(tok, prefix, port_map));
        } else {
            parts.push((*tok).to_string());
        }
    }
    parts.join(" ")
}

/// Scans `key=value` / `key = value` pairs of a card (parentheses and commas count as blanks).
fn scan_key_values(text: &str, params: &FxHashMap<String, f64>) -> Vec<KvPair> {
    let cleaned = text.replace(['(', ')', ','], " ");
    let toks: Vec<&str> = cleaned.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if let Some((k, v)) = toks[i].split_once('=') {
            if !v.is_empty() {
                out.push(KvPair {
                    key: k.trim().to_ascii_lowercase(),
                    raw: v.to_string(),
                    val: parse_spice_number(v, params),
                });
            } else if i + 1 < toks.len() {
                let val = parse_spice_number(toks[i + 1], params);
                out.push(KvPair {
                    key: k.trim().to_ascii_lowercase(),
                    raw: toks[i + 1].to_string(),
                    val,
                });
                if val.is_some() {
                    i += 1;
                }
            } else {
                out.push(KvPair {
                    key: k.trim().to_ascii_lowercase(),
                    raw: String::new(),
                    val: None,
                });
            }
        } else if i + 2 < toks.len() && toks[i + 1] == "=" {
            let val = parse_spice_number(toks[i + 2], params);
            out.push(KvPair {
                key: toks[i].trim().to_ascii_lowercase(),
                raw: toks[i + 2].to_string(),
                val,
            });
            if val.is_some() {
                i += 2;
            }
        }
        i += 1;
    }
    out
}

fn kv_map(pairs: &[KvPair]) -> FxHashMap<String, f64> {
    let mut kv = FxHashMap::default();
    for p in pairs {
        if let Some(v) = p.val {
            kv.insert(p.key.clone(), v);
        }
    }
    kv
}

fn extract_key_values(text: &str, params: &FxHashMap<String, f64>) -> FxHashMap<String, f64> {
    kv_map(&scan_key_values(text, params))
}

fn parse_model_card(
    line_no: usize,
    line: &str,
    netlist: &mut Netlist,
    sink: &mut DiagSink,
) -> SimResult<()> {
    let normalized = line.replace('(', " ( ").replace(')', " ) ");
    let toks: Vec<&str> = normalized.split_whitespace().collect();
    if toks.len() < 3 {
        return Err(SimError::ParseError {
            line: line_no,
            message: "Invalid .model syntax".to_string(),
        });
    }
    let mname = toks[1].to_ascii_lowercase();
    let mtype = toks[2].to_ascii_uppercase();
    let pairs = scan_key_values(line, &netlist.params);
    let kv = kv_map(&pairs);

    if mtype == "D" {
        let mut model = DiodeModelParams {
            name: toks[1].to_string(),
            ..Default::default()
        };
        if let Some(&v) = kv.get("is") {
            model.is = v;
        }
        if let Some(&v) = kv.get("n") {
            model.n = v;
        }
        if let Some(&v) = kv.get("rs") {
            model.rs = v;
        }
        if let Some(&v) = kv.get("cjo") {
            model.cjo = v;
        }
        if let Some(&v) = kv.get("vj") {
            model.vj = v;
        }
        if let Some(&v) = kv.get("m") {
            model.m = v;
        }
        if let Some(&v) = kv.get("tt") {
            model.tt = v;
        }
        model.eg = kv.get("eg").copied();
        model.xti = kv.get("xti").copied();
        model.tnom = kv.get("tnom").copied();
        audit_model_params(toks[1], "D", &pairs, &netlist.params, line_no, sink);
        netlist.diode_models.insert(mname, model);
    } else if mtype == "NMOS" || mtype == "PMOS" {
        let mos_type = if mtype == "PMOS" {
            MosfetType::Pmos
        } else {
            MosfetType::Nmos
        };
        let mut model = MosfetModelParams {
            name: toks[1].to_string(),
            mos_type,
            ..Default::default()
        };
        if let Some(&v) = kv.get("level") {
            model.level = v as u32;
        }
        if let Some(&v) = kv.get("vto").or_else(|| kv.get("vt0")) {
            model.vto = v;
        }
        if let Some(&v) = kv.get("kp").or_else(|| kv.get("uo")) {
            model.kp = v;
        }
        if let Some(&v) = kv.get("gamma") {
            model.gamma = v;
        }
        if let Some(&v) = kv.get("phi") {
            model.phi = v;
        }
        if let Some(&v) = kv.get("lambda") {
            model.lambda = v;
        }
        if let Some(&v) = kv.get("cgso") {
            model.cgso = v;
        }
        if let Some(&v) = kv.get("cgdo") {
            model.cgdo = v;
        }
        if let Some(&v) = kv.get("cgbo") {
            model.cgbo = v;
        }
        if let Some(&v) = kv.get("w") {
            model.w_default = v;
        }
        if let Some(&v) = kv.get("l") {
            model.l_default = v;
        }
        model.tnom = kv.get("tnom").copied();
        audit_model_params(toks[1], &mtype, &pairs, &netlist.params, line_no, sink);
        netlist.mosfet_models.insert(mname, model);
    } else if [
        "NPN", "PNP", "NJF", "PJF", "NMF", "PMF", "SW", "CSW", "R", "C", "L", "URC", "LTRA",
        "VDMOS", "TXL", "CPL",
    ]
    .contains(&mtype.as_str())
    {
        sink.warn(
            line_no,
            code::IGNORED_DIRECTIVE,
            format!(
                "`.model {} {}`: model type `{}` is not supported (only D, NMOS and PMOS are); the card was ignored",
                toks[1], toks[2], toks[2]
            ),
        );
    } else {
        sink.fatal(
            line_no,
            format!(
                "`.model {} {}`: unknown model type `{}`",
                toks[1], toks[2], toks[2]
            ),
        );
    }
    Ok(())
}

/// Pseudo-code of a source-reader issue that refuses the deck (see [`DiagSink::fatal`]).
const REFUSE: &str = "refuse";

/// Waveform keywords an independent source may carry.
const SOURCE_FUNCS: [&str; 9] = [
    "pulse", "sin", "exp", "pwl", "sffm", "am", "trnoise", "trrandom", "external",
];

/// Splits a source specification into tokens: blanks, commas and parentheses separate (each parenthesis is a
/// token of its own) and `name = value` is glued back into `name=value`.
fn source_tokens(spec: &str) -> Vec<String> {
    let spaced = spec
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace(',', " ")
        .replace('=', " = ");
    let raw: Vec<&str> = spaced.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "=" {
            if let (Some(prev), Some(next)) = (out.last_mut(), raw.get(i + 1)) {
                prev.push('=');
                prev.push_str(next);
                i += 2;
                continue;
            }
        }
        out.push(raw[i].to_string());
        i += 1;
    }
    out
}

/// Reads the specification of an independent source (everything after its two nodes):
/// `[[DC] <value>] [AC <mag> [<phase>]] [DISTOF1/2 ...] [PULSE|SIN|EXP|PWL|SFFM (<args>) [r=<t>] [td=<t>]]`.
/// Parentheses are optional and an unclosed list runs to the end of the card (both as in ngspice). Returns the
/// waveform and the reader diagnostics (code, message).
fn parse_source_spec(
    name: &str,
    spec: &str,
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) -> (SourceWaveform, Vec<(&'static str, String)>) {
    let toks = source_tokens(spec);
    let mut issues: Vec<(&'static str, String)> = Vec::new();
    let num = |t: &str| parse_spice_number(t, params);
    for t in &toks {
        if t.as_bytes()[0].is_ascii_digit() || t.starts_with(['.', '-', '+']) {
            audit_number(t, params, name, "value", line, sink);
        }
    }
    let mut explicit_dc: Option<f64> = None;
    let mut leading: Option<f64> = None;
    let mut func: Option<(String, Vec<f64>)> = None;
    let mut repeat: Option<f64> = None;
    let mut delay: Option<f64> = None;
    let mut i = 0;
    // `r=` / `td=` options of PWL (inside or after the argument list)
    let mut option = |tok: &str, issues: &mut Vec<(&'static str, String)>| -> bool {
        let lower = tok.to_ascii_lowercase();
        let (slot, v) = if let Some(v) = lower.strip_prefix("r=") {
            (&mut repeat, v.to_string())
        } else if let Some(v) = lower.strip_prefix("td=") {
            (&mut delay, v.to_string())
        } else {
            return false;
        };
        match parse_spice_number(&v, params) {
            Some(x) => *slot = Some(x),
            None => issues.push((
                REFUSE,
                format!("{name}: `{tok}` is not a number and was ignored"),
            )),
        }
        true
    };
    while i < toks.len() {
        let tok = toks[i].as_str();
        let lower = tok.to_ascii_lowercase();
        if lower == "dc" {
            match toks.get(i + 1).and_then(|t| num(t)) {
                Some(v) => {
                    explicit_dc = Some(v);
                    i += 2;
                }
                None => {
                    issues.push((
                        REFUSE,
                        format!("{name}: `DC` is not followed by a number; the DC value defaults"),
                    ));
                    i += 1;
                }
            }
            continue;
        }
        if let Some(v) = lower.strip_prefix("dc=") {
            match num(v) {
                Some(x) => explicit_dc = Some(x),
                None => issues.push((
                    REFUSE,
                    format!("{name}: `{tok}` is not a number and was ignored"),
                )),
            }
            i += 1;
            continue;
        }
        if lower == "ac" || lower == "distof1" || lower == "distof2" {
            // small-signal attributes: magnitude and optional phase (no AC analysis exists to use them)
            i += 1;
            for _ in 0..2 {
                if toks.get(i).and_then(|t| num(t)).is_some() {
                    i += 1;
                }
            }
            continue;
        }
        if SOURCE_FUNCS.contains(&lower.as_str()) {
            i += 1;
            let mut args: Vec<f64> = Vec::new();
            if toks.get(i).map(String::as_str) == Some("(") {
                i += 1;
                let mut closed = false;
                while i < toks.len() {
                    let t = toks[i].as_str();
                    i += 1;
                    if t == ")" {
                        closed = true;
                        break;
                    }
                    if option(t, &mut issues) {
                        continue;
                    }
                    match num(t) {
                        Some(v) => args.push(v),
                        None => issues.push((
                            REFUSE,
                            format!(
                                "{name}: {} argument `{t}` is not a number and was dropped",
                                lower.to_ascii_uppercase()
                            ),
                        )),
                    }
                }
                if !closed {
                    issues.push((
                        code::MALFORMED_INPUT,
                        format!("{name}: the {} argument list is not closed by `)`; it was read to the end of the card", lower.to_ascii_uppercase()),
                    ));
                }
            } else {
                while let Some(v) = toks.get(i).and_then(|t| num(t)) {
                    args.push(v);
                    i += 1;
                }
            }
            while let Some(t) = toks.get(i) {
                if !option(t, &mut issues) {
                    break;
                }
                i += 1;
            }
            if func.is_some() {
                issues.push((
                    REFUSE,
                    format!(
                        "{name}: a second waveform `{}` was ignored",
                        lower.to_ascii_uppercase()
                    ),
                ));
            } else {
                func = Some((lower, args));
            }
            continue;
        }
        if leading.is_none() && func.is_none() && explicit_dc.is_none() {
            if let Some(v) = num(tok) {
                leading = Some(v);
                i += 1;
                continue;
            }
        }
        issues.push((
            REFUSE,
            format!(
                "{name}: `{tok}` in the source specification was not understood and was ignored"
            ),
        ));
        i += 1;
    }

    let dc = explicit_dc.or(leading);
    let Some((fname, a)) = func else {
        return (SourceWaveform::Dc(dc.unwrap_or(0.0)), issues);
    };
    let upper = fname.to_ascii_uppercase();
    let arg = |k: usize| a.get(k).copied();
    let need = |n: usize, issues: &mut Vec<(&'static str, String)>| {
        if a.len() < n {
            issues.push((
                REFUSE,
                format!(
                    "{name}: {upper} needs at least {n} arguments, {} given; the source is the constant {}",
                    a.len(),
                    dc.unwrap_or(0.0)
                ),
            ));
            false
        } else {
            true
        }
    };
    let extra = |n: usize, issues: &mut Vec<(&'static str, String)>| {
        if a.len() > n {
            issues.push((
                code::IGNORED_WAVEFORM,
                format!(
                    "{name}: {} {upper} argument(s) beyond the {n}th were ignored",
                    a.len() - n
                ),
            ));
        }
    };
    // a zero or negative rise / fall time or period means "use the default", as in SPICE
    let positive_or_default = |v: Option<f64>| v.filter(|x| *x > 0.0).unwrap_or(f64::NAN);
    let mut w = match fname.as_str() {
        "pulse" if need(2, &mut issues) => {
            extra(8, &mut issues);
            SourceWaveform::Pulse {
                dc_val: 0.0,
                v1: a[0],
                v2: a[1],
                td: arg(2).unwrap_or(0.0),
                tr: positive_or_default(arg(3)),
                tf: positive_or_default(arg(4)),
                pw: arg(5).unwrap_or(f64::NAN),
                per: positive_or_default(arg(6)),
            }
        }
        "sin" if need(2, &mut issues) => {
            extra(6, &mut issues);
            SourceWaveform::Sin {
                dc_val: 0.0,
                vo: a[0],
                va: a[1],
                freq: arg(2).unwrap_or(f64::NAN),
                td: arg(3).unwrap_or(0.0),
                theta: arg(4).unwrap_or(0.0),
                phase: arg(5).unwrap_or(0.0),
            }
        }
        "exp" if need(2, &mut issues) => {
            extra(6, &mut issues);
            SourceWaveform::Exp {
                dc_val: 0.0,
                v1: a[0],
                v2: a[1],
                td1: arg(2).unwrap_or(0.0),
                tau1: positive_or_default(arg(3)),
                td2: arg(4).unwrap_or(f64::NAN),
                tau2: positive_or_default(arg(5)),
            }
        }
        "sffm" if need(2, &mut issues) => {
            extra(7, &mut issues);
            SourceWaveform::Sffm {
                dc_val: 0.0,
                vo: a[0],
                va: a[1],
                fc: positive_or_default(arg(2)),
                mdi: arg(3).unwrap_or(0.0),
                fs: positive_or_default(arg(4)),
                phasec: arg(5).unwrap_or(0.0),
                phases: arg(6).unwrap_or(0.0),
            }
        }
        "pwl" if need(2, &mut issues) => {
            if a.len() % 2 == 1 {
                issues.push((
                    REFUSE,
                    format!("{name}: PWL has an odd number of values; the last one was ignored"),
                ));
            }
            let points: Vec<(f64, f64)> = a.chunks_exact(2).map(|p| (p[0], p[1])).collect();
            if points.windows(2).any(|w| w[1].0 < w[0].0) {
                issues.push((
                    REFUSE,
                    format!(
                        "{name}: PWL time points are not in increasing order (SPICE rejects them)"
                    ),
                ));
            }
            SourceWaveform::Pwl {
                dc_val: 0.0,
                points,
                repeat,
                delay: delay.unwrap_or(0.0),
            }
        }
        "pulse" | "sin" | "exp" | "sffm" | "pwl" => SourceWaveform::Dc(dc.unwrap_or(0.0)),
        _ => {
            issues.push((
                REFUSE,
                format!(
                    "{name}: `{upper}(...)` waveforms are not supported; the source is the constant {} (SPICE evaluates the waveform)",
                    dc.unwrap_or(0.0)
                ),
            ));
            SourceWaveform::Dc(dc.unwrap_or(0.0))
        }
    };
    if (repeat.is_some() || delay.is_some()) && !matches!(w, SourceWaveform::Pwl { .. }) {
        issues.push((
            REFUSE,
            format!("{name}: `r=` / `td=` only apply to PWL and were ignored"),
        ));
    }
    // .op / .dc value: the DC attribute, else the value at t = 0 (SFFM: 0, as in ngspice)
    let at0 = if matches!(w, SourceWaveform::Sffm { .. }) {
        0.0
    } else {
        w.evaluate_at(0.0)
    };
    let dc_value = dc.unwrap_or(at0);
    match &mut w {
        SourceWaveform::Pulse { dc_val, .. }
        | SourceWaveform::Sin { dc_val, .. }
        | SourceWaveform::Exp { dc_val, .. }
        | SourceWaveform::Pwl { dc_val, .. }
        | SourceWaveform::Sffm { dc_val, .. } => *dc_val = dc_value,
        SourceWaveform::Dc(_) => {}
    }
    (w, issues)
}

/// Splits the fields of a `.ic` card into `(node, value)` pairs of `v(node)=value` assignments, accepting blanks
/// around `=` and inside the parentheses (`V( out ) = 5`). Fields that are not such an assignment are returned
/// separately (verbatim).
pub(crate) fn split_ic_assignments(fields: &[&str]) -> (Vec<(String, String)>, Vec<String>) {
    let mut joined = fields.join(" ");
    for (from, to) in [
        (" =", "="),
        ("= ", "="),
        ("( ", "("),
        (" (", "("),
        (" )", ")"),
    ] {
        while joined.contains(from) {
            joined = joined.replace(from, to);
        }
    }
    let mut pairs = Vec::new();
    let mut rejected = Vec::new();
    for tok in joined.split_whitespace() {
        let parsed = tok
            .get(..2)
            .filter(|p| p.eq_ignore_ascii_case("v("))
            .and_then(|_| tok[2..].split_once(")="))
            .filter(|(node, val)| !node.is_empty() && !val.is_empty() && !node.contains(','));
        match parsed {
            Some((node, val)) => pairs.push((node.to_string(), val.to_string())),
            None => rejected.push(tok.to_string()),
        }
    }
    (pairs, rejected)
}

fn parse_statement(
    line_no: usize,
    line: &str,
    netlist: &mut Netlist,
    sink: &mut DiagSink,
    state: &mut ParseState,
) -> SimResult<()> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.is_empty() {
        return Ok(());
    }
    let first = toks[0];
    if let Some(cmd) = first.strip_prefix('.') {
        let cmd_lower = cmd.to_ascii_lowercase();
        match cmd_lower.as_str() {
            "op" => {
                netlist.analyses.push(AnalysisCmd::Op);
            }
            "dc" => {
                if toks.len() != 5 && toks.len() != 9 {
                    return Err(SimError::ParseError {
                        line: line_no,
                        message: format!(
                            ".dc takes <var> <start> <stop> <step> [<var2> <start2> <stop2> <step2>], {} field(s) given",
                            toks.len() - 1
                        ),
                    });
                }
                let mut sweeps = Vec::with_capacity(2);
                for chunk in toks[1..].chunks(4) {
                    let mut vals = [0.0; 3];
                    for (k, field) in ["start", "stop", "step"].iter().enumerate() {
                        let raw = chunk[k + 1];
                        vals[k] = parse_spice_number(raw, &netlist.params).ok_or_else(|| {
                            SimError::ParseError {
                                line: line_no,
                                message: format!("Invalid .dc {field} value `{raw}`"),
                            }
                        })?;
                        audit_number(raw, &netlist.params, ".dc", field, line_no, sink);
                    }
                    sweeps.push(DcSweep {
                        name: chunk[0].to_string(),
                        start: vals[0],
                        stop: vals[1],
                        step: vals[2],
                    });
                }
                let outer = sweeps.get(1).cloned();
                let inner = sweeps.swap_remove(0);
                netlist.analyses.push(AnalysisCmd::Dc {
                    source_name: inner.name,
                    start: inner.start,
                    stop: inner.stop,
                    step: inner.step,
                    outer,
                });
            }
            "tran" => {
                if toks.len() < 3 {
                    return Err(SimError::ParseError {
                        line: line_no,
                        message: ".tran requires <tstep> <tstop>".to_string(),
                    });
                }
                let tstep = parse_spice_number(toks[1], &netlist.params).ok_or_else(|| {
                    SimError::ParseError {
                        line: line_no,
                        message: "Invalid .tran tstep".to_string(),
                    }
                })?;
                let tstop = parse_spice_number(toks[2], &netlist.params).ok_or_else(|| {
                    SimError::ParseError {
                        line: line_no,
                        message: "Invalid .tran tstop".to_string(),
                    }
                })?;
                // optional positional fields after tstop; `uic` may appear anywhere after them
                let optional: Vec<&str> = toks[3..]
                    .iter()
                    .copied()
                    .filter(|t| !t.eq_ignore_ascii_case("uic"))
                    .collect();
                let tstart = optional
                    .first()
                    .and_then(|s| parse_spice_number(s, &netlist.params))
                    .unwrap_or(0.0);
                let tmax = optional
                    .get(1)
                    .and_then(|s| parse_spice_number(s, &netlist.params));
                audit_number(toks[1], &netlist.params, ".tran", "tstep", line_no, sink);
                audit_number(toks[2], &netlist.params, ".tran", "tstop", line_no, sink);
                let uic = audit_tran(&toks, tstop, &netlist.params, line_no, sink);
                if uic {
                    state.uic_line.get_or_insert(line_no);
                }
                netlist.analyses.push(AnalysisCmd::Tran {
                    tstep,
                    tstop,
                    tstart,
                    tmax,
                    uic,
                });
            }
            "temp" => {
                let vals: Vec<Option<f64>> = toks[1..]
                    .iter()
                    .map(|t| parse_spice_number(t, &netlist.params))
                    .collect();
                match vals.as_slice() {
                    [Some(t)] => netlist.temp = Some(check_temperature(*t, "`.temp`", line_no)?),
                    [] | [None] => sink.warn(
                        line_no,
                        code::MALFORMED_INPUT,
                        format!("`{}` has no numeric temperature and was ignored", clip(line)),
                    ),
                    _ => sink.warn(
                        line_no,
                        code::IGNORED_TEMPERATURE,
                        format!(
                            "`{}`: temperature lists are not supported (one temperature per run); the circuit stays at {} °C",
                            clip(line),
                            netlist.temp.unwrap_or(crate::temperature::DEFAULT_TEMP_C)
                        ),
                    ),
                }
            }
            "options" | "option" | "opt" => {
                for (k, v) in scan_key_values(&toks[1..].join(" "), &netlist.params)
                    .into_iter()
                    .filter_map(|p| p.val.map(|v| (p.key, v)))
                {
                    match k.as_str() {
                        "temp" => {
                            netlist.temp = Some(check_temperature(v, "`.options temp`", line_no)?)
                        }
                        "tnom" => {
                            netlist.tnom = Some(check_temperature(v, "`.options tnom`", line_no)?)
                        }
                        "gmin" if v >= 0.0 && v.is_finite() => netlist.gmin = Some(v),
                        "gmin" => sink.fatal(
                            line_no,
                            format!("`.options gmin={v}` is not a valid conductance"),
                        ),
                        _ => {}
                    }
                }
                audit_ignored_dot_card(&cmd_lower, &toks, &netlist.params, line_no, sink);
            }
            "nodeset" => {
                let (pairs, rejected) = split_ic_assignments(&toks[1..]);
                audit_ic(&pairs, &rejected, &netlist.params, line_no, sink);
                for (node, val) in &pairs {
                    if let Some(v) = parse_spice_number(val, &netlist.params) {
                        let node_id = netlist.intern_node(node);
                        netlist.nodesets.insert(node_id, v);
                    }
                }
            }
            "ic" => {
                let (pairs, rejected) = split_ic_assignments(&toks[1..]);
                audit_ic(&pairs, &rejected, &netlist.params, line_no, sink);
                for (node, val) in &pairs {
                    if let Some(v) = parse_spice_number(val, &netlist.params) {
                        let node_id = netlist.intern_node(node);
                        netlist.initial_conditions.insert(node_id, v);
                    }
                }
            }
            _ => {
                // Everything else (`.end`, `.options`, `.print`, `.save`, `.plot`, `.temp`, ...) is not
                // interpreted; each ignored card is reported (never silently dropped).
                audit_ignored_dot_card(&cmd_lower, &toks, &netlist.params, line_no, sink);
            }
        }
        return Ok(());
    }

    if !state.seen_names.insert(name_hash(first)) {
        sink.fatal(
            line_no,
            format!("element name `{first}` is used more than once (SPICE rejects the netlist)"),
        );
    }

    let lead = first.as_bytes()[0].to_ascii_uppercase();
    match lead {
        b'R' => {
            if toks.len() < 4 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Resistor requires `R<name> <n+> <n-> <value>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let val_str = toks[3]
                .strip_prefix("r=")
                .or_else(|| toks[3].strip_prefix("R="))
                .unwrap_or(toks[3]);
            let resistance = parse_spice_number(val_str, &netlist.params).ok_or_else(|| {
                SimError::ParseError {
                    line: line_no,
                    message: format!("Invalid resistance value `{}`", toks[3]),
                }
            })?;
            audit_nodes(first, &toks[1..3], line_no, sink);
            audit_number(val_str, &netlist.params, first, "value", line_no, sink);
            if resistance == 0.0 {
                sink.warn(
                    line_no,
                    code::MALFORMED_INPUT,
                    format!("{first}: resistance {val_str} is zero; RustSpice substitutes 1e-6 ohm (SPICE rejects R=0)"),
                );
            }
            audit_element_tail(Elem::R, first, &toks[4..], &netlist.params, line_no, sink);
            let kv = extract_key_values(&toks[4..].join(" "), &netlist.params);
            // m parallel copies
            let resistance = resistance / instance_multiplier(&kv, first, line_no, sink);
            PendingTc::record(state, LinearKind::R, netlist.resistors.len(), &kv, line_no);
            netlist.resistors.push(ResistorInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                resistance,
            });
        }
        b'C' => {
            if toks.len() < 4 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Capacitor requires `C<name> <n+> <n-> <value>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let capacitance = parse_spice_number(toks[3], &netlist.params).ok_or_else(|| {
                SimError::ParseError {
                    line: line_no,
                    message: format!("Invalid capacitance value `{}`", toks[3]),
                }
            })?;
            audit_nodes(first, &toks[1..3], line_no, sink);
            audit_number(toks[3], &netlist.params, first, "value", line_no, sink);
            audit_element_tail(Elem::C, first, &toks[4..], &netlist.params, line_no, sink);
            let kv = extract_key_values(line, &netlist.params);
            let capacitance = capacitance * instance_multiplier(&kv, first, line_no, sink);
            PendingTc::record(state, LinearKind::C, netlist.capacitors.len(), &kv, line_no);
            if let Some(ic) = kv.get("ic") {
                state
                    .pending_elem_ic
                    .push((line_no, format!("{first}: ic={ic}")));
            }
            netlist.capacitors.push(CapacitorInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                capacitance,
                ic: kv.get("ic").copied(),
            });
        }
        b'L' => {
            if toks.len() < 4 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Inductor requires `L<name> <n+> <n-> <value>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let inductance = parse_spice_number(toks[3], &netlist.params).ok_or_else(|| {
                SimError::ParseError {
                    line: line_no,
                    message: format!("Invalid inductance value `{}`", toks[3]),
                }
            })?;
            audit_nodes(first, &toks[1..3], line_no, sink);
            audit_number(toks[3], &netlist.params, first, "value", line_no, sink);
            audit_element_tail(Elem::L, first, &toks[4..], &netlist.params, line_no, sink);
            let kv = extract_key_values(line, &netlist.params);
            // m parallel inductors
            let inductance = inductance / instance_multiplier(&kv, first, line_no, sink);
            PendingTc::record(state, LinearKind::L, netlist.inductors.len(), &kv, line_no);
            if let Some(ic) = kv.get("ic") {
                state
                    .pending_elem_ic
                    .push((line_no, format!("{first}: ic={ic}")));
            }
            netlist.inductors.push(InductorInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                inductance,
                ic: kv.get("ic").copied(),
            });
        }
        b'G' => {
            if toks.len() < 6 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "VCCS requires `G<name> <n+> <n-> <nc+> <nc-> <gm>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let nc_pos = netlist.intern_node(toks[3]);
            let nc_neg = netlist.intern_node(toks[4]);
            let gm = parse_spice_number(toks[5], &netlist.params).ok_or_else(|| {
                SimError::ParseError {
                    line: line_no,
                    message: format!("Invalid VCCS transconductance `{}`", toks[5]),
                }
            })?;
            audit_nodes(first, &toks[1..5], line_no, sink);
            audit_number(toks[5], &netlist.params, first, "gm", line_no, sink);
            audit_element_tail(Elem::G, first, &toks[6..], &netlist.params, line_no, sink);
            let kv = extract_key_values(&toks[6..].join(" "), &netlist.params);
            let gm = gm * instance_multiplier(&kv, first, line_no, sink);
            netlist.vccs.push(VccsInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                nc_pos,
                nc_neg,
                gm,
            });
        }
        b'V' => {
            if toks.len() < 3 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Voltage source requires `V<name> <n+> <n-> <spec>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let (waveform, issues) =
                parse_source_spec(first, &toks[3..].join(" "), &netlist.params, line_no, sink);
            audit_nodes(first, &toks[1..3], line_no, sink);
            for (c, msg) in issues {
                if c == REFUSE {
                    sink.fatal(line_no, msg);
                } else {
                    sink.warn(line_no, c, msg);
                }
            }
            netlist.v_sources.push(VoltageSourceInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                waveform,
            });
        }
        b'I' => {
            if toks.len() < 3 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Current source requires `I<name> <n+> <n-> <spec>`".to_string(),
                });
            }
            let n_pos = netlist.intern_node(toks[1]);
            let n_neg = netlist.intern_node(toks[2]);
            let (waveform, issues) =
                parse_source_spec(first, &toks[3..].join(" "), &netlist.params, line_no, sink);
            audit_nodes(first, &toks[1..3], line_no, sink);
            for (c, msg) in issues {
                if c == REFUSE {
                    sink.fatal(line_no, msg);
                } else {
                    sink.warn(line_no, c, msg);
                }
            }
            netlist.i_sources.push(CurrentSourceInstance {
                name: first.to_string(),
                n_pos,
                n_neg,
                waveform,
            });
        }
        b'D' => {
            if toks.len() < 4 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "Diode requires `D<name> <anode> <cathode> <model>`".to_string(),
                });
            }
            let n_anode = netlist.intern_node(toks[1]);
            let n_cathode = netlist.intern_node(toks[2]);
            let model = toks[3].to_ascii_lowercase();
            audit_nodes(first, &toks[1..3], line_no, sink);
            if !netlist.diode_models.contains_key(&model) {
                report_undefined_model(
                    first,
                    toks[3],
                    "diode",
                    netlist.mosfet_models.contains_key(&model),
                    "MOSFET",
                    line_no,
                    sink,
                );
            }
            netlist
                .diode_models
                .entry(model.clone())
                .or_insert_with(|| DiodeModelParams {
                    name: toks[3].to_string(),
                    ..Default::default()
                });
            audit_element_tail(Elem::D, first, &toks[4..], &netlist.params, line_no, sink);
            let kv = extract_key_values(&toks[4..].join(" "), &netlist.params);
            // `area=` or the positional area right after the model name
            let positional_area = toks
                .get(4)
                .filter(|t| !t.contains('='))
                .and_then(|t| parse_spice_number(t, &netlist.params));
            let area = kv.get("area").copied().or(positional_area).unwrap_or(1.0)
                * instance_multiplier(&kv, first, line_no, sink);
            netlist.diodes.push(DiodeInstance {
                name: first.to_string(),
                n_anode,
                n_cathode,
                model,
                area,
                temp: kv.get("temp").copied(),
                dtemp: kv.get("dtemp").copied(),
            });
        }
        b'M' => {
            if toks.len() < 6 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: "MOSFET requires `M<name> <nd> <ng> <ns> <nb> <model>`".to_string(),
                });
            }
            let n_drain = netlist.intern_node(toks[1]);
            let n_gate = netlist.intern_node(toks[2]);
            let n_source = netlist.intern_node(toks[3]);
            let n_bulk = netlist.intern_node(toks[4]);
            let model = toks[5].to_ascii_lowercase();
            audit_nodes(first, &toks[1..5], line_no, sink);
            if toks[5].contains('=') {
                sink.fatal(
                    line_no,
                    format!("{first}: `{}` is in the model-name position: a node (usually the bulk) is missing from the card", toks[5]),
                );
            } else if !netlist.mosfet_models.contains_key(&model) {
                report_undefined_model(
                    first,
                    toks[5],
                    "MOSFET",
                    netlist.diode_models.contains_key(&model),
                    "diode",
                    line_no,
                    sink,
                );
            }
            netlist
                .mosfet_models
                .entry(model.clone())
                .or_insert_with(|| MosfetModelParams {
                    name: toks[5].to_string(),
                    mos_type: if model.starts_with('p') {
                        MosfetType::Pmos
                    } else {
                        MosfetType::Nmos
                    },
                    ..Default::default()
                });
            audit_element_tail(Elem::M, first, &toks[6..], &netlist.params, line_no, sink);
            let kv = extract_key_values(&toks[6..].join(" "), &netlist.params);
            let mult = instance_multiplier(&kv, first, line_no, sink);
            let w = kv.get("w").copied();
            let l = kv.get("l").copied();
            if let Some(l_val) = l {
                if l_val <= 0.0 {
                    sink.warn(
                        line_no,
                        code::MALFORMED_INPUT,
                        format!("{first}: L={l_val:.4e} is not positive; SPICE rejects non-positive channel length"),
                    );
                }
            }
            netlist.mosfets.push(MosfetInstance {
                name: first.to_string(),
                n_drain,
                n_gate,
                n_source,
                n_bulk,
                model,
                w,
                l,
                m: mult,
                temp: kv.get("temp").copied(),
                dtemp: kv.get("dtemp").copied(),
            });
        }
        _ => {
            return Err(SimError::ParseError {
                line: line_no,
                message: format!("Unsupported device card `{first}`"),
            });
        }
    }
    Ok(())
}

/// The instance multiplier `m=` (number of parallel copies, default 1). A non-positive value is refused with a
/// diagnostic and treated as 1.
fn instance_multiplier(
    kv: &FxHashMap<String, f64>,
    name: &str,
    line: usize,
    sink: &mut DiagSink,
) -> f64 {
    match kv.get("m").copied() {
        None => 1.0,
        Some(m) if m > 0.0 && m.is_finite() => m,
        Some(m) => {
            sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!("{name}: multiplier m={m} is not positive; m=1 is used (SPICE rejects it)"),
            );
            1.0
        }
    }
}

/// A device names a model that has no `.model` card of the right type: the reader silently substitutes defaults.
fn report_undefined_model(
    device: &str,
    model: &str,
    dev_kind: &str,
    declared_as_other: bool,
    other_kind: &str,
    line: usize,
    sink: &mut DiagSink,
) {
    if declared_as_other {
        sink.fatal(
            line,
            format!("{device}: {dev_kind} uses model `{model}`, which is declared as a {other_kind} model (SPICE rejects the mismatch)"),
        );
    } else {
        sink.fatal(
            line,
            format!(
                "{device}: model `{model}` is not defined (no `.model` card of a supported type)"
            ),
        );
    }
}
