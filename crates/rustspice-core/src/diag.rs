//! Reader diagnostics: every construct the SPICE reader accepts but does not (fully) honour is reported
//! here instead of being dropped silently (AGENTS.md Rule 7: never silently ignore a semantic construct).
//!
//! The mechanism is deliberately *non-invasive*: the parsing code keeps its exact numerical behaviour and
//! the audit functions below look at the same tokens and say what the reader threw away or re-interpreted.
//! A diagnostic never changes the parsed [`crate::netlist::Netlist`], never changes an exit code and is
//! bounded (at most [`MAX_DIAGNOSTICS`] stored per deck, the rest is counted).

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::netlist::{number_prefix_len, parse_spice_number};

/// Upper bound on stored diagnostics per deck (Rule 9: no unbounded growth on hostile input).
pub const MAX_DIAGNOSTICS: usize = 100;
/// Longest stored diagnostic message in bytes (longer ones are cut and end in `...`).
pub const MAX_MESSAGE_BYTES: usize = 400;

/// Stable diagnostic codes (kebab-case). One code per defect class of the benchmark slug registry.
pub mod code {
    /// `.include/.lib/.global/.nodeset/.if/...` accepted and ignored, unknown cards, unsupported `.model` types.
    pub const IGNORED_DIRECTIVE: &str = "ignored-directive";
    /// `.options` accepted and ignored.
    pub const IGNORED_OPTION: &str = "ignored-option";
    /// `.ac/.tf/.noise/.sens/.pz/.disto/.step/.meas/.four/.control` dropped (DC `.op` substituted).
    pub const IGNORED_ANALYSIS: &str = "ignored-analysis";
    /// Model parameter parsed and dropped (or approximated).
    pub const IGNORED_MODEL_PARAM: &str = "ignored-model-param";
    /// Instance parameter parsed and dropped.
    pub const IGNORED_INSTANCE_PARAM: &str = "ignored-instance-param";
    /// `LEVEL >= 2` evaluated with a different model.
    pub const WRONG_MODEL_LEVEL: &str = "wrong-model-level";
    /// Source waveform not supported / argument dropped.
    pub const IGNORED_WAVEFORM: &str = "ignored-waveform";
    /// `.temp`, `tnom`, `temp=`, `dtemp=`, `tc1/tc2` ignored (fixed 300.15 K).
    pub const IGNORED_TEMPERATURE: &str = "ignored-temperature";
    /// `m=` multiplier / positional diode area ignored.
    pub const IGNORED_MULTIPLIER: &str = "ignored-multiplier";
    /// `uic`, element `ic=`, spaced `.ic` ignored or only partly applied.
    pub const IGNORED_IC: &str = "ignored-ic";
    /// `.tran tstart` / `tmax` parsed and dropped.
    pub const IGNORED_TRAN_OPTION: &str = "ignored-tran-option";
    /// `{2*rv}`-style expression cut to its leading number, unparsable argument dropped.
    pub const EXPR_TRUNCATION: &str = "expr-truncation";
    /// Valid syntax read as something else.
    pub const MISPARSE: &str = "misparse";
    /// Malformed input accepted (unknown parameter, extra tokens, unbalanced delimiters, ...).
    pub const MALFORMED_INPUT: &str = "malformed-input";
    /// First line handling differs from SPICE (line 1 is always the title).
    pub const TITLE_LINE: &str = "title-line";
    /// Model referenced by a device is not defined (defaults silently used).
    pub const UNDEFINED_MODEL: &str = "undefined-model";
    /// `.dc` leaves the swept source overwritten for the next analysis / nested sweeps dropped.
    pub const DC_SWEEP: &str = "dc-sweep";
    /// `DC <v>` attribute dropped when a waveform is present.
    pub const DC_VALUE_DROPPED: &str = "dc-value-dropped";
    /// Diagnostic budget exhausted.
    pub const SUPPRESSED: &str = "suppressed";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// A semantic construct was dropped, approximated or re-interpreted: results may differ from SPICE.
    Warning,
    /// Informational: the construct has no effect on the numbers RustSpice produces (output selection ...).
    Note,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 1-based source line (0 = whole deck).
    pub line: usize,
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

impl Diagnostic {
    /// `warning[code]: line N: message` (the form printed on stderr).
    pub fn render(&self) -> String {
        let sev = match self.severity {
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        if self.line == 0 {
            format!("{sev}[{}]: {}", self.code, self.message)
        } else {
            format!("{sev}[{}]: line {}: {}", self.code, self.line, self.message)
        }
    }
}

/// Bounded, de-duplicating collector (the same source line can be expanded many times by `.subckt`).
#[derive(Debug, Default)]
pub struct DiagSink {
    items: Vec<Diagnostic>,
    seen: FxHashSet<(usize, String)>,
    suppressed: usize,
    /// First input the reader refuses (SPICE rejects it, and no lenient reading is defensible).
    fatal: Option<(usize, String)>,
}

impl DiagSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a refusal: the deck is rejected with this message (the first one wins) once reading ends.
    pub fn fatal(&mut self, line: usize, message: impl Into<String>) {
        if self.fatal.is_none() {
            self.fatal = Some((line, message.into()));
        }
    }

    /// The refusal recorded by [`DiagSink::fatal`], if any.
    pub fn take_fatal(&mut self) -> Option<(usize, String)> {
        self.fatal.take()
    }

    fn push(&mut self, line: usize, severity: Severity, code: &str, mut message: String) {
        if self.items.len() >= MAX_DIAGNOSTICS {
            self.suppressed += 1;
            return;
        }
        if message.len() > MAX_MESSAGE_BYTES {
            // hostile input can echo 100 KB tokens: keep every stored diagnostic small
            let mut end = MAX_MESSAGE_BYTES;
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            message.truncate(end);
            message.push_str("...");
        }
        if !self.seen.insert((line, message.clone())) {
            return;
        }
        self.items.push(Diagnostic {
            line,
            severity,
            code: code.to_string(),
            message,
        });
    }

    /// True when a diagnostic with this code was already collected.
    pub fn has(&self, code: &str) -> bool {
        self.items.iter().any(|d| d.code == code)
    }

    pub fn warn(&mut self, line: usize, code: &str, message: impl Into<String>) {
        self.push(line, Severity::Warning, code, message.into());
    }

    pub fn note(&mut self, line: usize, code: &str, message: impl Into<String>) {
        self.push(line, Severity::Note, code, message.into());
    }

    /// Finishes the collection (source order, whole-deck notes last, a trailing note reports what was dropped).
    pub fn finish(mut self) -> Vec<Diagnostic> {
        self.items
            .sort_by_key(|d| if d.line == 0 { usize::MAX } else { d.line });
        if self.suppressed > 0 {
            self.items.push(Diagnostic {
                line: 0,
                severity: Severity::Note,
                code: code::SUPPRESSED.to_string(),
                message: format!(
                    "{} further diagnostics suppressed (limit {MAX_DIAGNOSTICS} per deck)",
                    self.suppressed
                ),
            });
        }
        self.items
    }
}

// ---------------------------------------------------------------------------------------------------------
// numbers
// ---------------------------------------------------------------------------------------------------------

/// True for a plain numeric literal that needs no diagnostic: a numeric prefix followed by an alphanumeric suffix
/// that does not start with `x` (read as `meg`), and a finite value. Never allocates.
pub(crate) fn is_clean_literal(t: &str) -> bool {
    let b = t.as_bytes();
    let end = number_prefix_len(b);
    if end == 0 {
        return false; // empty, `{...}`, quotes, names
    }
    let sfx = &b[end..];
    if !sfx.iter().all(|c| c.is_ascii_alphanumeric())
        || sfx.first().is_some_and(|c| *c == b'x' || *c == b'X')
    {
        return false;
    }
    // `1..2`, `-`, `.` are scanned as numeric prefixes but do not parse; only exponents can overflow
    let (mut dots, mut digit, mut exp) = (0usize, false, false);
    for c in &b[..end] {
        match c {
            b'.' => dots += 1,
            b'0'..=b'9' => digit = true,
            b'e' | b'E' => exp = true,
            _ => {}
        }
    }
    if dots > 1 || !digit {
        return false;
    }
    if exp || end > 300 {
        return t[..end]
            .parse::<f64>()
            .map(|v| v.is_finite())
            .unwrap_or(false);
    }
    true
}

/// Numeric value of a field for the audit passes. Literals that start with a digit, dot or sign cannot be
/// parameter names, so the (allocating) parameter lookup of [`parse_spice_number`] is skipped for them.
fn num_value(tok: &str, params: &FxHashMap<String, f64>) -> Option<f64> {
    match tok.as_bytes().first() {
        Some(c) if c.is_ascii_digit() || *c == b'.' => {
            literal_value(tok.trim_end_matches(['}', '\'', '"']))
        }
        _ => parse_spice_number(tok, params),
    }
}

/// Same scale-factor table as `parse_spice_number`, without allocating.
fn literal_value(t: &str) -> Option<f64> {
    let b = t.as_bytes();
    let end = number_prefix_len(b);
    if end == 0 {
        return None;
    }
    let base: f64 = t[..end].parse().ok()?;
    let sfx = &b[end..];
    let lc = |i: usize| sfx.get(i).map(|c| c.to_ascii_lowercase());
    let mult = if sfx.is_empty() {
        1.0
    } else if (lc(0) == Some(b'm') && lc(1) == Some(b'e') && lc(2) == Some(b'g'))
        || lc(0) == Some(b'x')
    {
        1e6
    } else if lc(0) == Some(b'm') && lc(1) == Some(b'i') && lc(2) == Some(b'l') {
        25.4e-6
    } else {
        match lc(0).unwrap_or(0) {
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

/// Looks at the raw text of a numeric field that [`parse_spice_number`] accepted and reports what the lenient
/// scanner threw away: trailing arithmetic (`{2*rv}` -> `2`), the `x` suffix read as `meg`, unbalanced
/// delimiters, values that overflow to infinity.
pub(crate) fn audit_number(
    raw: &str,
    params: &FxHashMap<String, f64>,
    owner: &str,
    field: &str,
    line: usize,
    sink: &mut DiagSink,
) {
    let t = raw.trim();
    if t.is_empty() {
        return;
    }
    // cheap exit for the overwhelmingly common clean literal (`1k`, `0.5`, `1e-9`, `3.3V`): nothing to report
    if is_clean_literal(t) {
        return;
    }
    let what = format!("{owner} {field}");
    let what = what.as_str();
    if t.matches('{').count() != t.matches('}').count() {
        sink.warn(
            line,
            code::MALFORMED_INPUT,
            format!("{what}: unbalanced braces in `{t}`; the stray delimiter is stripped and the rest is used"),
        );
    } else if t.matches('\'').count() % 2 == 1 {
        sink.warn(
            line,
            code::MALFORMED_INPUT,
            format!("{what}: unterminated quote in `{t}`; the stray quote is stripped and the rest is used"),
        );
    }
    let inner = t.trim_matches(|c| c == '{' || c == '}' || c == '\'' || c == '"');
    if inner.is_empty() || params.contains_key(&inner.to_ascii_lowercase()) {
        return;
    }
    let end = number_prefix_len(inner.as_bytes());
    if end == 0 {
        return;
    }
    let (prefix, suffix) = inner.split_at(end);
    if let Ok(v) = prefix.parse::<f64>() {
        if !v.is_finite() {
            sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!("{what}: `{t}` overflows to a non-finite value"),
            );
        }
    }
    if suffix.is_empty() {
        return;
    }
    if suffix.bytes().any(|b| {
        matches!(
            b,
            b'*' | b'/'
                | b'+'
                | b'-'
                | b'^'
                | b'('
                | b')'
                | b'<'
                | b'>'
                | b'?'
                | b':'
                | b','
                | b'%'
                | b'='
                | b'&'
                | b'|'
                | b'!'
        )
    }) {
        sink.warn(
            line,
            code::EXPR_TRUNCATION,
            format!(
                "{what}: `{t}` is read as the number `{prefix}` with the trailing `{suffix}` discarded (arithmetic expressions are not evaluated)"
            ),
        );
    } else if suffix.as_bytes()[0].eq_ignore_ascii_case(&b'x') {
        sink.warn(
            line,
            code::MISPARSE,
            format!("{what}: suffix `x` in `{t}` is read as `meg` (1e6); SPICE treats it as unit text (factor 1)"),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// dot cards
// ---------------------------------------------------------------------------------------------------------

/// Reports a dot card that reached the catch-all of the statement parser (it is ignored).
pub(crate) fn audit_ignored_dot_card(
    cmd: &str,
    toks: &[&str],
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) {
    let card = toks[0];
    match cmd {
        "end" => {}
        "ac" | "tf" | "noise" | "sens" | "pz" | "disto" => sink.fatal(
            line,
            format!("`{card}` analysis is not supported (only .op, .dc and .tran are): refusing the deck rather than running a different analysis"),
        ),
        "step" => sink.fatal(
            line,
            "`.step` is not supported: refusing the deck rather than simulating only the unstepped values",
        ),
        "meas" | "measure" | "four" | "fourier" => sink.warn(
            line,
            code::IGNORED_ANALYSIS,
            format!("`{card}` post-processing is not supported and was skipped"),
        ),
        "options" | "option" | "opt" => audit_options(toks, params, line, sink),
        "global" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!(
                "`.global {}` inside a `.subckt` body was ignored (declare global nodes at the top level)",
                toks[1..].join(" ")
            ),
        ),
        "hdl" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!(
                "`{}` is not supported: the file is not read, so its models, subcircuits and cards are missing",
                toks.join(" ")
            ),
        ),
        "if" | "elseif" | "else" | "endif" => sink.fatal(
            line,
            format!("`{card}`: conditional blocks are not supported (refusing rather than instantiating every branch)"),
        ),
        "func" | "csparam" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!("`{card}` is not supported and was ignored"),
        ),
        "param" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            "`.param` inside a .subckt body is not supported and was ignored",
        ),
        "model" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            "`.model` inside a .subckt body is not supported and was ignored: devices that use it get default parameters",
        ),
        "ends" => sink.warn(
            line,
            code::MALFORMED_INPUT,
            "`.ends` without a matching `.subckt` was ignored",
        ),
        "print" | "plot" | "probe" | "save" | "width" | "title" => sink.note(
            line,
            code::IGNORED_DIRECTIVE,
            format!("`{card}` is not supported: output selection is ignored (all vectors are written)"),
        ),
        _ => sink.fatal(line, format!("unknown card `{card}` (SPICE rejects unknown cards)")),
    }
}

fn audit_options(toks: &[&str], params: &FxHashMap<String, f64>, line: usize, sink: &mut DiagSink) {
    let cleaned = toks[1..]
        .join(" ")
        .replace(" = ", "=")
        .replace("= ", "=")
        .replace(" =", "=");
    let mut ignored: Vec<String> = Vec::new();
    let mut output_only: Vec<String> = Vec::new();
    for tok in cleaned.split_whitespace() {
        let (k, v) = tok.split_once('=').unwrap_or((tok, ""));
        let key = k.to_ascii_lowercase();
        match key.as_str() {
            "temp" | "tnom" | "gmin" => {
                if parse_spice_number(v, params).is_none() {
                    sink.fatal(line, format!("`.options {tok}`: the value is not a number"));
                }
            }
            "list" | "nopage" | "node" | "post" | "nomod" | "acct" | "noacct" | "opts"
            | "numdgt" | "width" => output_only.push(tok.to_string()),
            _ => ignored.push(tok.to_string()),
        }
    }
    if !ignored.is_empty() {
        sink.warn(
            line,
            code::IGNORED_OPTION,
            format!(
                "`.options {}` is ignored: RustSpice runs with fixed built-in settings (reltol 1e-6, vntol 1e-10, abstol 1e-12)",
                ignored.join(" ")
            ),
        );
    }
    if !output_only.is_empty() {
        sink.note(
            line,
            code::IGNORED_OPTION,
            format!(
                "`.options {}` is ignored (output formatting only)",
                output_only.join(" ")
            ),
        );
    }
}

/// `.tran <tstep> <tstop> [tstart [tmax]] [uic]`: checks the optional fields; returns whether `uic` is present.
pub(crate) fn audit_tran(
    toks: &[&str],
    tstop: f64,
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) -> bool {
    let mut uic_seen = false;
    let mut pos: Vec<&str> = Vec::new();
    for t in &toks[3.min(toks.len())..] {
        if t.eq_ignore_ascii_case("uic") {
            uic_seen = true;
        } else {
            pos.push(*t);
        }
    }
    if let Some(t) = pos.first() {
        match parse_spice_number(t, params) {
            Some(v) => {
                audit_number(t, params, ".tran", "tstart", line, sink);
                if v > tstop {
                    sink.warn(
                        line,
                        code::MALFORMED_INPUT,
                        format!("`.tran` tstart={t} exceeds tstop; SPICE rejects this card"),
                    );
                }
            }
            None => sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!("`.tran` field `{t}` is not a number and was ignored"),
            ),
        }
    }
    if let Some(t) = pos.get(1) {
        match parse_spice_number(t, params) {
            Some(v) => {
                audit_number(t, params, ".tran", "tmax", line, sink);
                if v <= 0.0 || v.is_nan() {
                    sink.warn(
                        line,
                        code::IGNORED_TRAN_OPTION,
                        format!("`.tran` tmax={t} is not positive and was ignored"),
                    );
                }
            }
            None => sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!("`.tran` field `{t}` is not a number and was ignored"),
            ),
        }
    }
    if pos.len() > 2 {
        sink.warn(
            line,
            code::MALFORMED_INPUT,
            format!("`.tran` extra fields `{}` were ignored", pos[2..].join(" ")),
        );
    }
    uic_seen
}

/// `.ic`: `v(node)=value` assignments (blanks around `=` allowed); anything else is reported.
pub(crate) fn audit_ic(
    pairs: &[(String, String)],
    rejected: &[String],
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) {
    let mut bad: Vec<String> = rejected.to_vec();
    for (node, val) in pairs {
        if parse_spice_number(val, params).is_none() {
            bad.push(format!("v({node})={val}"));
        } else {
            audit_number(val, params, ".ic", node, line, sink);
        }
    }
    if !bad.is_empty() {
        sink.fatal(
            line,
            format!(
                "`.ic` fields `{}` are not understood (expected `v(node)=value` with a numeric value)",
                bad.join(" ")
            ),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// models
// ---------------------------------------------------------------------------------------------------------

/// One `key=value` occurrence of a card (value text and its parse result).
#[derive(Clone, Debug)]
pub(crate) struct KvPair {
    pub key: String,
    pub raw: String,
    pub val: Option<f64>,
}

/// Reports the model parameters the reader drops (or only approximates) and the unparsable values.
pub(crate) fn audit_model_params(
    model: &str,
    mtype: &str,
    pairs: &[KvPair],
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) {
    let honored: &[&str] = match mtype {
        "D" => &[
            "is", "n", "rs", "cjo", "cj0", "vj", "m", "tt", "eg", "xti", "tnom", "bv", "ibv",
            "ikf", "isr", "nr", "fc",
        ],
        "R" => &["rsh", "narrow", "short", "defw"],
        "C" => &["cj", "cjsw", "narrow", "short", "defw"],
        _ => &[
            "level", "vto", "vt0", "kp", "gamma", "phi", "lambda", "cgso", "cgdo", "cgbo", "w",
            "l", "tnom", "tox", "uo", "u0", "nsub", "nss", "tpg", "ld", "rd", "rs", "rsh", "is",
            "js", "cbd", "cbs", "cj", "cjsw", "mj", "mjsw", "pb", "fc",
        ],
    };
    let is_mos = mtype == "NMOS" || mtype == "PMOS";
    let mut unmodelled: Vec<String> = Vec::new();
    for p in pairs {
        if p.key.is_empty() {
            continue;
        }
        let k = p.key.as_str();
        if p.val.is_none() && !honored.contains(&k) && k != "tnom" && !p.raw.is_empty() {
            // text-valued parameter of an unsupported model (VERSION=4.8.0, ...): just not modelled
            unmodelled.push(p.key.to_ascii_uppercase());
            continue;
        }
        if p.val.is_none() {
            let shown = if p.raw.is_empty() {
                "(no value)"
            } else {
                p.raw.as_str()
            };
            sink.fatal(
                line,
                format!(
                    "model `{model}`: value `{shown}` of parameter `{}` is not a number",
                    p.key.to_ascii_uppercase()
                ),
            );
            continue;
        }
        let up_key = p.key.to_ascii_uppercase();
        audit_number(
            &p.raw,
            params,
            &format!("model `{model}`"),
            &up_key,
            line,
            sink,
        );
        let up = p.key.to_ascii_uppercase();
        if k == "tpg" && is_mos && p.val.is_some_and(|v| v != 0.0 && v.abs() != 1.0) {
            sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!(
                    "model `{model}`: TPG={} is not +1, -1 or 0; its sign is used",
                    p.raw
                ),
            );
        }
        if k == "level" && is_mos {
            if p.val.unwrap_or(1.0) >= 2.0 {
                sink.fatal(
                    line,
                    format!(
                        "model `{model}`: MOSFET LEVEL={} is not implemented (only LEVEL=1 is)",
                        p.raw
                    ),
                );
            }
        } else if k == "phi" && is_mos {
            if p.val.unwrap_or(1.0) <= 0.0 {
                sink.warn(
                    line,
                    code::MALFORMED_INPUT,
                    format!("model `{model}`: PHI={} is not positive; SPICE rejects the model, RustSpice uses it as is", p.raw),
                );
            }
        } else if !honored.contains(&k) {
            unmodelled.push(up);
        }
    }
    if !unmodelled.is_empty() {
        sink.warn(
            line,
            code::IGNORED_MODEL_PARAM,
            format!(
                "model `{model}` ({mtype}): parameter(s) `{}` are not modelled and were ignored",
                unmodelled.join(" ")
            ),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// element instance parameters
// ---------------------------------------------------------------------------------------------------------

/// Element kinds with their honoured keys and the SPICE instance parameters they may legitimately carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Elem {
    R,
    C,
    L,
    G,
    D,
    M,
    /// Controlled sources `E`, `F`, `H` and the coupling `K`: no instance parameters.
    Controlled,
}

impl Elem {
    fn honored(self) -> &'static [&'static str] {
        match self {
            Elem::R => &["r", "w", "l", "m", "tc1", "tc2", "temp", "dtemp"],
            Elem::C => &["w", "l", "m", "tc1", "tc2", "temp", "dtemp"],
            Elem::L => &["m", "tc1", "tc2", "temp", "dtemp"],
            Elem::G => &["m"],
            Elem::D => &["area", "m", "temp", "dtemp"],
            Elem::M => &[
                "w", "l", "m", "temp", "dtemp", "ad", "as", "pd", "ps", "nrd", "nrs",
            ],
            Elem::Controlled => &[],
        }
    }

    fn known(self) -> &'static [&'static str] {
        match self {
            Elem::R => &["r", "w", "l", "ac", "scale", "noisy"],
            Elem::C => &["c", "w", "l", "scale"],
            Elem::L => &["l", "nt", "scale"],
            Elem::G => &["ac"],
            Elem::D => &[
                "off", "pj", "sens", "thermal", "lm", "lp", "wm", "wp", "xm", "xp",
            ],
            Elem::M => &["off", "geo", "delvto", "sens", "dtemp_off"],
            Elem::Controlled => &[],
        }
    }
}

fn key_in(k: &str, list: &[&str]) -> bool {
    list.iter().any(|x| k.eq_ignore_ascii_case(x))
}

/// Reports every token of an element card after the fields the reader consumes.
pub(crate) fn audit_element_tail(
    kind: Elem,
    name: &str,
    tail: &[&str],
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) {
    if tail.is_empty() {
        return;
    }
    // `key = value` written with blanks is re-joined (rare); the usual `key=value` tokens are used as they are
    let spaced = tail
        .iter()
        .any(|t| *t == "=" || t.starts_with('=') || t.ends_with('='));
    let joined: String;
    let mut it_joined;
    let mut it_plain;
    let iter: &mut dyn Iterator<Item = &str> = if spaced {
        joined = tail
            .join(" ")
            .replace(" = ", "=")
            .replace("= ", "=")
            .replace(" =", "=");
        it_joined = joined.split_whitespace();
        &mut it_joined
    } else {
        it_plain = tail.iter().copied();
        &mut it_plain
    };
    let mut positional = 0usize;
    let mut extras: Vec<&str> = Vec::new();
    for tok in iter {
        if let Some((k, v)) = tok.split_once('=') {
            if key_in(k, kind.honored()) {
                if num_value(v, params).is_none() {
                    sink.fatal(
                        line,
                        format!("{name}: value `{v}` of `{k}` is not a number"),
                    );
                } else {
                    audit_number(v, params, name, k, line, sink);
                }
            } else if key_in(k, &["tce"]) {
                sink.warn(
                    line,
                    code::IGNORED_TEMPERATURE,
                    format!("{name}: `{tok}` is ignored: the exponential temperature coefficient is not modelled"),
                );
            } else if k.eq_ignore_ascii_case("ic") {
                // C/L `ic=` is reported by the caller (its severity depends on whether `uic` is given)
                if !matches!(kind, Elem::C | Elem::L) {
                    sink.warn(
                        line,
                        code::IGNORED_IC,
                        format!("{name}: `{tok}` is ignored"),
                    );
                }
            } else if key_in(k, kind.known()) {
                sink.warn(
                    line,
                    code::IGNORED_INSTANCE_PARAM,
                    format!("{name}: parameter `{k}` is not modelled and was ignored"),
                );
            } else {
                sink.fatal(
                    line,
                    format!("{name}: unknown instance parameter `{k}` (SPICE rejects it)"),
                );
            }
        } else {
            positional += 1;
            if kind == Elem::D && positional == 1 && num_value(tok, params).is_some() {
                // positional area: honoured by the reader
            } else if matches!(kind, Elem::D | Elem::M) && tok.eq_ignore_ascii_case("off") {
                sink.warn(
                    line,
                    code::IGNORED_INSTANCE_PARAM,
                    format!("{name}: `off` is not modelled and was ignored"),
                );
            } else {
                extras.push(tok);
            }
        }
    }
    if !extras.is_empty() {
        sink.fatal(
            line,
            format!(
                "{name}: unexpected token(s) `{}` after the value",
                extras.join(" ")
            ),
        );
    }
}

/// Node names that contain `(`, `)` or `=` are almost certainly unsupported syntax read as a node
/// (`poly(1)`, `w=10u` in the model position, ...).
pub(crate) fn audit_nodes(name: &str, nodes: &[&str], line: usize, sink: &mut DiagSink) {
    for n in nodes {
        if n.bytes().any(|c| c == b'(' || c == b')' || c == b'=') {
            sink.fatal(
                line,
                format!("{name}: `{n}` in a node position looks like unsupported syntax (e.g. POLY(...)) or a missing field"),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// independent sources
// ---------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::netlist::parse_netlist;

    fn codes(deck: &str) -> Vec<String> {
        parse_netlist(deck)
            .unwrap()
            .diagnostics
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    fn has(deck: &str, code: &str) -> bool {
        codes(deck).iter().any(|c| c == code)
    }

    #[test]
    fn clean_deck_is_silent() {
        let d = "t\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 2.2k\nC1 out 0 1n\nD1 out 0 DM\n.model DM D(IS=1e-14 N=1.1)\n.op\n.end\n";
        assert!(codes(d).is_empty(), "{:?}", codes(d));
    }

    #[test]
    fn ignored_constructs_are_reported() {
        let base = "t\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 1k\n";
        for (extra, want) in [
            (".options reltol=1e-4\n.op", "ignored-option"),
            (".temp 0 27 85\n.op", "ignored-temperature"),
            (".tran 1n 1u 0 -1n", "ignored-tran-option"),
            ("R3 out 0 1k m=-1\n.op", "malformed-input"),
            ("R3 out 0 1k tce=0.1\n.op", "ignored-temperature"),
            ("R3 out 0 1x\n.op", "misparse"),
            (".model QX NPN(BF=100)\n.op", "ignored-directive"),
            (
                ".model DM D(KF=1e-12 AF=1)\nD9 out 0 DM\n.op",
                "ignored-model-param",
            ),
            ("V9 a 0 PULSE(0 1 0 1n 1n 1u\n.op", "malformed-input"),
            ("V9 a 0 SIN(0 1 1k 0 0 90 7)\n.op", "ignored-waveform"),
            (".control\nrun\n.endc\n.op", "ignored-analysis"),
        ] {
            let deck = format!("{base}{extra}\n.end\n");
            assert!(
                has(&deck, want),
                "`{extra}` should report `{want}`, got {:?}",
                codes(&deck)
            );
        }
    }

    #[test]
    fn malformed_or_unsupported_input_is_refused() {
        // SPICE rejects these, or no lenient reading is defensible: the deck fails with a parse error
        let base = "t\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 1k\n";
        for (extra, want) in [
            (
                ".include rustspice_no_such_models.lib\n.op",
                "rustspice_no_such_models.lib",
            ),
            (".frob 1 2\n.op", "unknown card"),
            (".ac dec 10 1 1k", ".ac"),
            (".step param k 1 2 1\n.op", ".step"),
            (".ic V(out) = 1 junk\n.tran 1n 1u", ".ic"),
            ("R3 out 0 1k\nR3 out 0 1k\n.op", "more than once"),
            ("R3 out 0 1k foo=2\n.op", "foo"),
            ("R3 out 0 1k 2 3\n.op", "unexpected token"),
            ("D9 out 0 NOPE\n.op", "NOPE"),
            (
                ".model DX DD(IS=1e-12)\nD9 out 0 DX\n.op",
                "unknown model type",
            ),
            (".model DX D(IS=oops)\nD9 out 0 DX\n.op", "IS"),
            (".model DX NMOS(VTO=1)\nD9 out 0 DX\n.op", "declared as"),
            (
                ".model MX NMOS(LEVEL=3 VTO=1)\nM1 out in 0 0 MX W=1u L=1u\n.op",
                "LEVEL=3",
            ),
            (
                "M1 out in 0 MX W=1u L=1u\n.model MX NMOS(VTO=1)\n.op",
                "model-name position",
            ),
            ("V9 a 0 AM(1 1 1k 1meg 0)\n.op", "AM"),
            ("V9 a 0 PWL(0 0 1u)\n.op", "PWL"),
            ("V9 a 0 PWL()\n.op", "PWL"),
            ("V9 a 0 nan\n.op", "nan"),
            (".param a={b+1} b={a+1}\n.op", ".param"),
            (".param x=\n.op", ".param"),
            (".subckt foo a b\nR9 a b 1k\n.op", ".ends"),
            (".options gmin=abc\n.op", "gmin"),
            ("R3 out 0 1k \u{FFFD}\n.op", "UTF-8"),
        ] {
            let deck = format!("{base}{extra}\n.end\n");
            match parse_netlist(&deck) {
                Ok(n) => panic!(
                    "`{extra}` should be refused, got diagnostics {:?}",
                    n.diagnostics.iter().map(|d| &d.code).collect::<Vec<_>>()
                ),
                Err(e) => assert!(e.to_string().contains(want), "`{extra}`: {e}"),
            }
        }
    }

    #[test]
    fn harmless_forms_stay_silent() {
        let base = "t\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 1k\n";
        for extra in [
            // implemented: tstart / tmax / uic / spaced .ic
            ".tran 1n 1u 0 10n",
            ".tran 1n 1u 100n",
            ".tran 1n 1u uic",
            ".tran 1n 1u 0 10n uic",
            ".ic V(out) = 1\n.tran 1n 1u",
            ".ic v( out )=1 V(in) = 2\n.tran 1n 1u",
            ".temp 27\n.op",
            ".options temp=27\n.op",
            ".temp 85\n.options tnom=50\n.op",
            "R3 out 0 1k tc1=0.01 tc2=1e-5 dtemp=10\n.op",
            "R3 out 0 1k5\n.op",
            "C1 out 0 10uF\n.op",
            "V9 a 0 PULSE(0 1 0 1n 1n 1u 2u)\n.op",
            "V9 a 0 SIN(0 1 1k 0 0 0)\n.op",
            "V9 a 0 DC 0 PULSE(0 1 0 1n 1n 1u 2u)\n.op",
            ".param k=3\nR3 out 0 {k}\n.op",
            ".param k = 3 j={k*2}\nR3 out 0 {k + j}\n.op",
            "V9 a 0 PWL(0 0 1u 1) r=0\n.op",
            "V9 a 0 EXP(0 1 1u 300n 2u 500n)\n.op",
            "V9 a 0 SFFM(0 1 5meg 3 500k)\n.op",
            "V9 a 0 SIN(0 1 1k 0 0 90)\n.op",
            "V9 a 0 DC 2 SIN(1 1 1k)\n.op",
            "V9 a 0 PULSE 0 5 1u 100n 100n 1u 3u\n.op",
            ".options gmin=1e-9\n.op",
            ".nodeset v(out)=1\n.op",
            ".global vdd\n.op",
            ".dc V1 0 1 0.5 R1 1k 2k 1k",
            ".dc temp 0 50 25",
        ] {
            let deck = format!("{base}{extra}\n.end\n");
            assert!(
                codes(&deck).is_empty(),
                "`{extra}` should be silent, got {:?}",
                codes(&deck)
            );
        }
    }

    #[test]
    fn diagnostics_never_change_the_circuit() {
        // a lenient reading is kept as is: only a diagnostic is added
        let n = parse_netlist(
            "t\n.param rv=1k\nV1 in 0 PWL(0 0.5 1u 1)\nR1 in out {2*rv}\nR2 out 0 1x\n.op\n",
        )
        .unwrap();
        assert_eq!(n.resistors[0].resistance, 2e3);
        assert_eq!(n.resistors[1].resistance, 1e6);
        assert_eq!(n.v_sources[0].waveform.dc_value(), 0.5);
        assert!(!n.diagnostics.is_empty());
    }

    #[test]
    fn diagnostics_are_bounded() {
        let mut deck = String::from("t\nV1 in 0 1\n");
        for i in 0..1000 {
            deck.push_str(&format!("R{i} in 0 1k tce=2\n"));
        }
        deck.push_str(".op\n");
        let n = parse_netlist(&deck).unwrap();
        assert!(n.diagnostics.len() <= super::MAX_DIAGNOSTICS + 1);
        assert!(n.diagnostics.last().unwrap().code == "suppressed");
    }

    #[test]
    fn fast_number_path_agrees_with_the_reader() {
        // `num_value` / `is_clean_literal` are allocation-free shortcuts of the audit pass: they must agree with
        // `parse_spice_number` (the function that decides the simulated value)
        let params = rustc_hash::FxHashMap::default();
        let mut cases: Vec<String> = Vec::new();
        for num in [
            "1", "0.5", ".5", "2.", "1e3", "1E-9", "4.7", "100", "1e999", "007",
        ] {
            for sfx in [
                "", "k", "K", "meg", "MEG", "Meg", "mil", "MIL", "m", "M", "u", "U", "n", "p", "f",
                "a", "t", "g", "x", "X", "xyz", "V", "ohm", "Hz", "s", "k5", "meg5", "*2", "/3",
                "+1", "-1", "(1)", "%", "e", "mi", "me",
            ] {
                cases.push(format!("{num}{sfx}"));
            }
        }
        cases.extend(
            [
                "1e", "1e+", "1e+3k", "--1", "1..2", "abc", "", "{1k}", "'1k'", "1k}",
            ]
            .map(String::from),
        );
        for c in &cases {
            let want = crate::netlist::parse_spice_number(c, &params);
            if c.as_bytes()
                .first()
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            {
                assert_eq!(
                    super::num_value(c, &params).map(f64::to_bits),
                    want.map(f64::to_bits),
                    "{c}"
                );
            }
            if super::is_clean_literal(c) {
                let v = want.expect(c);
                assert!(v.is_finite(), "{c}");
                let mut sink = super::DiagSink::new();
                super::audit_number(c, &params, "R1", "value", 1, &mut sink);
                assert!(sink.finish().is_empty(), "{c} is clean but reported");
            }
        }
    }

    #[test]
    fn title_line_handling() {
        // line 1 is the title even when it is blank: V1 on line 2 is a card
        let n = parse_netlist("\nV1 in 0 1\nR1 in 0 1k\n.op\n").unwrap();
        assert_eq!(n.v_sources.len(), 1);
        assert!(has(
            ".subckt x a b\nV1 1 0 5\nR1 1 0 1k\n.op\n",
            "title-line"
        ));
        assert!(!has("title\nV1 in 0 1\nR1 in 0 1k\n.op\n", "title-line"));
    }
}
