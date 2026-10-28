//! High-speed zero-copy SPICE netlist parser and programmatic [`CircuitBuilder`] API.

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::diag::{
    audit_dc, audit_element_tail, audit_ic, audit_ignored_dot_card, audit_model_params,
    audit_nodes, audit_number, audit_source, audit_tran, code, DiagSink, Diagnostic, Elem, KvPair,
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
    /// Damped sinusoid: `SIN(vo va freq td theta)`.
    Sin {
        dc_val: f64,
        vo: f64,
        va: f64,
        freq: f64,
        td: f64,
        theta: f64,
    },
}

impl SourceWaveform {
    /// Returns the source value during DC operating point analysis.
    #[inline]
    pub fn dc_value(&self) -> f64 {
        match self {
            Self::Dc(v) => *v,
            Self::Pulse { dc_val, .. } => *dc_val,
            Self::Sin { dc_val, .. } => *dc_val,
        }
    }

    /// Evaluates the instantaneous source value at simulation time `t` (in seconds).
    /// Note: at `t == 0.0` (initial transient operating point), matches SPICE `v1` / `vo`.
    #[inline]
    pub fn evaluate_at(&self, t: f64) -> f64 {
        match *self {
            Self::Dc(v) => v,
            Self::Pulse {
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
            Self::Sin {
                vo,
                va,
                freq,
                td,
                theta,
                ..
            } => {
                if t <= td {
                    vo
                } else {
                    let dt = t - td;
                    vo + va * (-dt * theta).exp() * (2.0 * std::f64::consts::PI * freq * dt).sin()
                }
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
    },
    /// Transient Analysis (`.tran <tstep> <tstop> [<tstart> [<tmax>]]`).
    Tran {
        tstep: f64,
        tstop: f64,
        tstart: f64,
        tmax: Option<f64>,
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
    pub analyses: Vec<AnalysisCmd>,
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
        fn waveform_fields(w: &SourceWaveform) -> [(&'static str, f64); 4] {
            const PAD: (&str, f64) = ("", 0.0);
            match *w {
                SourceWaveform::Dc(v) => [("dc value", v), PAD, PAD, PAD],
                SourceWaveform::Pulse { dc_val, v1, v2, .. } => [
                    ("PULSE dc value", dc_val),
                    ("PULSE v1", v1),
                    ("PULSE v2", v2),
                    PAD,
                ],
                SourceWaveform::Sin {
                    dc_val,
                    vo,
                    va,
                    freq,
                    ..
                } => [
                    ("SIN dc value", dc_val),
                    ("SIN vo", vo),
                    ("SIN va", va),
                    ("SIN frequency", freq),
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
                    start, stop, step, ..
                } => {
                    if let Some((f, v)) =
                        bad(&[("start", *start), ("stop", *stop), ("step", *step)])
                    {
                        return fail(".dc", f, v);
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
    lines: Vec<(usize, String)>,
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
    /// `DC <v>` attributes dropped next to a waveform (line, text): they only matter for .op/.dc.
    pending_dc_attr: Vec<(usize, String)>,
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
    let mut sink = DiagSink::new();
    let mut logical_lines: Vec<(usize, String)> = Vec::new();
    let mut title = String::from("Untitled SPICE Deck");
    let mut seen_title = false;
    let mut in_control_block = false;
    let mut control_start = 0usize;
    let mut control_lines = 0usize;
    let mut first_line_blank = false;
    let mut title_card_line: Option<usize> = None;

    for (idx, raw_line) in src.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if idx == 0 {
                first_line_blank = true;
            }
            continue;
        }
        if !seen_title {
            seen_title = true;
            if !trimmed.starts_with('.') && !trimmed.starts_with('*') && !trimmed.starts_with(';') {
                title = trimmed.to_string();
                if first_line_blank {
                    sink.warn(
                        line_no,
                        code::TITLE_LINE,
                        format!(
                            "line 1 is blank: SPICE takes line 1 as the title and simulates this line (`{}`) as a card; RustSpice used it as the title and does not simulate it",
                            clip(trimmed)
                        ),
                    );
                }
                continue;
            }
            if trimmed.starts_with('.') {
                title_card_line = Some(line_no);
                sink.warn(
                    line_no,
                    code::TITLE_LINE,
                    format!(
                        "the first line `{}` starts with `.` and is parsed as a card; SPICE always takes line 1 as the title",
                        clip(trimmed)
                    ),
                );
            }
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
        // Strip inline comments starting with `;` or `$`
        let clean = trimmed
            .split(';')
            .next()
            .unwrap_or("")
            .split('$')
            .next()
            .unwrap_or("")
            .trim();
        if clean.is_empty() {
            continue;
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
    let mut active_subckt: Option<(String, SubcktDef)> = None;
    let mut subckt_start = 0usize;
    let mut top_lines: Vec<(usize, String)> = Vec::new();

    // First pass: collect `.param`, `.model`, and `.subckt` definitions
    for (line_no, line) in logical_lines {
        let lower = line.to_ascii_lowercase();
        if let Some((_, ref mut sub)) = active_subckt {
            if lower.starts_with(".ends") {
                let (name, finished) = active_subckt.take().unwrap();
                subckts.insert(name, finished);
            } else {
                sub.lines.push((line_no, line));
            }
            continue;
        }

        if lower.starts_with(".subckt") {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 3 {
                return Err(SimError::ParseError {
                    line: line_no,
                    message: ".subckt requires a name and at least one port".to_string(),
                });
            }
            let name = toks[1].to_ascii_lowercase();
            let ports = toks[2..].iter().map(|s| (*s).to_string()).collect();
            active_subckt = Some((
                name,
                SubcktDef {
                    ports,
                    lines: Vec::new(),
                },
            ));
            subckt_start = line_no;
        } else if lower.starts_with(".param") {
            parse_param_card(line_no, &line[6..], &mut netlist.params, &mut sink);
        } else if lower.starts_with(".model") {
            parse_model_card(line_no, &line, &mut netlist, &mut sink)?;
        } else {
            top_lines.push((line_no, line));
        }
    }
    if let Some((name, def)) = &active_subckt {
        sink.warn(
            subckt_start,
            code::MALFORMED_INPUT,
            format!(
                "`.subckt {name}` has no matching `.ends`: its {} body lines were discarded",
                def.lines.len()
            ),
        );
    }

    // Expand subcircuits (`X...`) with an explicit stack: bounded by `limits`, never by the call stack
    let expanded_lines = expand_subcircuits(&top_lines, &subckts, limits)?;

    // Second pass: parse all elements and analysis cards
    let mut state = ParseState::default();
    state.seen_names.reserve(expanded_lines.len());
    for (line_no, line) in expanded_lines {
        parse_statement(line_no, &line, &mut netlist, &mut sink, &mut state)?;
    }

    if let Some(ln) = state.uic_line {
        if netlist.initial_conditions.is_empty() {
            sink.warn(
                ln,
                code::IGNORED_IC,
                "`uic` is ignored: the transient starts from the DC operating point, not from the zero / initial-condition state",
            );
        } else {
            sink.note(
                ln,
                code::IGNORED_IC,
                "`uic` is not implemented: the transient starts from the DC operating point and the `.ic` values are imposed on top of it",
            );
        }
    }
    let dc_matters = netlist
        .analyses
        .iter()
        .any(|a| matches!(a, AnalysisCmd::Op | AnalysisCmd::Dc { .. }))
        || netlist.analyses.is_empty();
    for (ln, msg) in std::mem::take(&mut state.pending_dc_attr) {
        if dc_matters {
            sink.warn(ln, code::DC_VALUE_DROPPED, msg);
        } else {
            sink.note(
                ln,
                code::DC_VALUE_DROPPED,
                format!(
                    "{msg} (no .op/.dc analysis in this deck, so the transient is not affected)"
                ),
            );
        }
    }
    for (ln, what) in std::mem::take(&mut state.pending_elem_ic) {
        if state.uic_line.is_some() {
            sink.warn(
                ln,
                code::IGNORED_IC,
                format!("{what} is parsed but never applied: the transient starts from the DC operating point (no UIC support)"),
            );
        } else {
            sink.note(
                ln,
                code::IGNORED_IC,
                format!("{what} is not applied (it only matters together with `.tran ... uic`, which RustSpice does not support)"),
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

    if let Some(ln) = title_card_line {
        // whatever else that first line produced is an artifact of it being parsed as a card
        sink.retain_only_code_on_line(ln, code::TITLE_LINE);
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
/// * a subcircuit instantiating itself, directly or through other subcircuits, is refused with the cycle
///   spelled out (without a parameter language such an expansion can never terminate);
/// * `limits.max_subckt_depth`, `limits.max_expanded_cards` (instance + element cards visited) and
///   `limits.max_expanded_bytes` (flattened text) are enforced while expanding.
fn expand_subcircuits(
    top_lines: &[(usize, String)],
    subckts: &FxHashMap<String, SubcktDef>,
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
    }];

    while let Some(frame) = stack.last_mut() {
        let lines: &[(usize, String)] = frame.lines;
        let Some((line_no, line)) = lines.get(frame.next) else {
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
        let first_char = line.as_bytes()[0].to_ascii_uppercase();
        if first_char == b'X' {
            let toks: Vec<&str> = line.split_whitespace().collect();
            if toks.len() < 3 {
                return Err(SimError::ParseError {
                    line: *line_no,
                    message: "Invalid X subcircuit instance card".to_string(),
                });
            }
            let inst_name = toks[0];
            let sub_name = toks.last().unwrap().to_ascii_lowercase();
            let (sub_key, sub) =
                subckts
                    .get_key_value(&sub_name)
                    .ok_or_else(|| SimError::ParseError {
                        line: *line_no,
                        message: format!("Unknown subcircuit `{sub_name}`"),
                    })?;
            let actual_nodes = &toks[1..toks.len() - 1];
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

fn parse_param_card(
    line_no: usize,
    rest: &str,
    params: &mut FxHashMap<String, f64>,
    sink: &mut DiagSink,
) {
    let spaced = rest.contains(" =") || rest.contains("= ");
    if spaced {
        sink.warn(
            line_no,
            code::MALFORMED_INPUT,
            "`.param` with blanks around `=` is not understood: write `name=value` (the affected parameters are not defined)",
        );
    }
    for token in rest.split_whitespace() {
        if let Some((k, v)) = token.split_once('=') {
            if let Some(num) = parse_spice_number(v, params) {
                audit_number(v, params, ".param", k, line_no, sink);
                params.insert(k.trim().to_ascii_lowercase(), num);
            } else if !spaced {
                sink.warn(
                    line_no,
                    code::MALFORMED_INPUT,
                    format!("`.param {token}`: the value is not a number (expressions are not evaluated); the parameter is not defined"),
                );
            }
        } else if !spaced {
            sink.warn(
                line_no,
                code::MALFORMED_INPUT,
                format!("`.param` token `{token}` was ignored (expected name=value)"),
            );
        }
    }
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
        audit_model_params(toks[1], &mtype, &pairs, &netlist.params, line_no, sink);
        netlist.mosfet_models.insert(mname, model);
    } else {
        sink.warn(
            line_no,
            code::IGNORED_DIRECTIVE,
            format!(
                "`.model {} {}`: model type `{}` is not supported (only D, NMOS and PMOS are); the card was ignored",
                toks[1], toks[2], toks[2]
            ),
        );
    }
    Ok(())
}

fn parse_waveform(
    tokens: &[&str],
    full_line: &str,
    params: &FxHashMap<String, f64>,
) -> SourceWaveform {
    let upper = full_line.to_ascii_uppercase();
    let mut dc_val = 0.0;

    for (idx, tok) in tokens.iter().enumerate() {
        if tok.eq_ignore_ascii_case("dc") {
            if let Some(next) = tokens.get(idx + 1) {
                if let Some(v) = parse_spice_number(next, params) {
                    dc_val = v;
                }
            }
        } else if idx == 0 {
            if let Some(v) = parse_spice_number(tok, params) {
                dc_val = v;
            }
        }
    }

    if let Some(p_idx) = upper.find("PULSE") {
        let after = &full_line[p_idx + 5..];
        if let (Some(l), Some(r)) = (after.find('('), after.find(')')) {
            if l < r {
                let args: Vec<f64> = after[l + 1..r]
                    .replace(',', " ")
                    .split_whitespace()
                    .filter_map(|s| parse_spice_number(s, params))
                    .collect();
                if args.len() >= 2 {
                    let v1 = args[0];
                    let v2 = args[1];
                    let td = args.get(2).copied().unwrap_or(0.0);
                    let tr = args.get(3).copied().unwrap_or(0.0);
                    let tf = args.get(4).copied().unwrap_or(0.0);
                    let pw = args.get(5).copied().unwrap_or(1e9);
                    let per = args.get(6).copied().unwrap_or(0.0);
                    return SourceWaveform::Pulse {
                        dc_val: v1,
                        v1,
                        v2,
                        td,
                        tr,
                        tf,
                        pw,
                        per,
                    };
                }
            }
        }
    }

    if let Some(s_idx) = upper.find("SIN") {
        let after = &full_line[s_idx + 3..];
        if let (Some(l), Some(r)) = (after.find('('), after.find(')')) {
            if l < r {
                let args: Vec<f64> = after[l + 1..r]
                    .replace(',', " ")
                    .split_whitespace()
                    .filter_map(|s| parse_spice_number(s, params))
                    .collect();
                if args.len() >= 3 {
                    let vo = args[0];
                    let va = args[1];
                    let freq = args[2];
                    let td = args.get(3).copied().unwrap_or(0.0);
                    let theta = args.get(4).copied().unwrap_or(0.0);
                    return SourceWaveform::Sin {
                        dc_val: vo,
                        vo,
                        va,
                        freq,
                        td,
                        theta,
                    };
                }
            }
        }
    }

    SourceWaveform::Dc(dc_val)
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
                if toks.len() < 5 {
                    return Err(SimError::ParseError {
                        line: line_no,
                        message: ".dc requires <src> <start> <stop> <step>".to_string(),
                    });
                }
                let source_name = toks[1].to_string();
                let start = parse_spice_number(toks[2], &netlist.params).ok_or_else(|| {
                    SimError::ParseError {
                        line: line_no,
                        message: "Invalid .dc start value".to_string(),
                    }
                })?;
                let stop = parse_spice_number(toks[3], &netlist.params).ok_or_else(|| {
                    SimError::ParseError {
                        line: line_no,
                        message: "Invalid .dc stop value".to_string(),
                    }
                })?;
                let step = parse_spice_number(toks[4], &netlist.params).ok_or_else(|| {
                    SimError::ParseError {
                        line: line_no,
                        message: "Invalid .dc step value".to_string(),
                    }
                })?;
                for (field, raw) in [("start", toks[2]), ("stop", toks[3]), ("step", toks[4])] {
                    audit_number(raw, &netlist.params, ".dc", field, line_no, sink);
                }
                audit_dc(&toks, line_no, sink);
                netlist.analyses.push(AnalysisCmd::Dc {
                    source_name,
                    start,
                    stop,
                    step,
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
                let tstart = toks
                    .get(3)
                    .and_then(|s| parse_spice_number(s, &netlist.params))
                    .unwrap_or(0.0);
                let tmax = toks
                    .get(4)
                    .and_then(|s| parse_spice_number(s, &netlist.params));
                audit_number(toks[1], &netlist.params, ".tran", "tstep", line_no, sink);
                audit_number(toks[2], &netlist.params, ".tran", "tstop", line_no, sink);
                if audit_tran(&toks, tstop, &netlist.params, line_no, sink) {
                    state.uic_line.get_or_insert(line_no);
                }
                netlist.analyses.push(AnalysisCmd::Tran {
                    tstep,
                    tstop,
                    tstart,
                    tmax,
                });
            }
            "ic" => {
                audit_ic(&toks, &netlist.params, line_no, sink);
                for tok in &toks[1..] {
                    let lower = tok.to_ascii_lowercase();
                    if let Some(rest) = lower.strip_prefix("v(") {
                        if let Some((node_str, val_str)) = rest.split_once(")=") {
                            let node_id = netlist.intern_node(node_str);
                            if let Some(v) = parse_spice_number(val_str, &netlist.params) {
                                netlist.initial_conditions.insert(node_id, v);
                            }
                        }
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
        sink.warn(
            line_no,
            code::MALFORMED_INPUT,
            format!("element name `{first}` is used more than once (SPICE rejects the netlist); both cards are kept"),
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
            let waveform = parse_waveform(&toks[3..], line, &netlist.params);
            audit_nodes(first, &toks[1..3], line_no, sink);
            if let Some(msg) = audit_source(
                first,
                line,
                &toks[3..],
                &waveform,
                &netlist.params,
                line_no,
                sink,
            ) {
                state.pending_dc_attr.push((line_no, msg));
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
            let waveform = parse_waveform(&toks[3..], line, &netlist.params);
            audit_nodes(first, &toks[1..3], line_no, sink);
            if let Some(msg) = audit_source(
                first,
                line,
                &toks[3..],
                &waveform,
                &netlist.params,
                line_no,
                sink,
            ) {
                state.pending_dc_attr.push((line_no, msg));
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
            let area = kv.get("area").copied().unwrap_or(1.0);
            netlist.diodes.push(DiodeInstance {
                name: first.to_string(),
                n_anode,
                n_cathode,
                model,
                area,
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
                sink.warn(
                    line_no,
                    code::MISPARSE,
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
        sink.warn(
            line,
            code::UNDEFINED_MODEL,
            format!("{device}: {dev_kind} uses model `{model}`, which is declared as a {other_kind} model; default {dev_kind} parameters are used (SPICE rejects the mismatch)"),
        );
    } else {
        sink.warn(
            line,
            code::UNDEFINED_MODEL,
            format!("{device}: model `{model}` is not defined (no usable `.model` card: it may sit in an unread .include/.lib or inside a .subckt); default {dev_kind} parameters are used"),
        );
    }
}
