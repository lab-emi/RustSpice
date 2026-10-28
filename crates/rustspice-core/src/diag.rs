//! Reader diagnostics: every construct the SPICE reader accepts but does not (fully) honour is reported
//! here instead of being dropped silently (AGENTS.md Rule 7: never silently ignore a semantic construct).
//!
//! The mechanism is deliberately *non-invasive*: the parsing code keeps its exact numerical behaviour and
//! the audit functions below look at the same tokens and say what the reader threw away or re-interpreted.
//! A diagnostic never changes the parsed [`crate::netlist::Netlist`], never changes an exit code and is
//! bounded (at most [`MAX_DIAGNOSTICS`] stored per deck, the rest is counted).

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::netlist::{number_prefix_len, parse_spice_number, SourceWaveform};

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
}

impl DiagSink {
    pub fn new() -> Self {
        Self::default()
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

    /// Drops every diagnostic of `line` except those with `keep_code` (used when a card is only an artifact of
    /// another diagnostic, e.g. a title line that the reader parsed as a card).
    pub fn retain_only_code_on_line(&mut self, line: usize, keep_code: &str) {
        self.items.retain(|d| d.line != line || d.code == keep_code);
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
        || sfx.first().map_or(false, |c| *c == b'x' || *c == b'X')
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
        "ac" | "tf" | "noise" | "sens" | "pz" | "disto" => sink.warn(
            line,
            code::IGNORED_ANALYSIS,
            format!("`{card}` analysis is not supported and was skipped (only .op/.dc/.tran run; a DC .op is substituted when none remains)"),
        ),
        "step" => sink.warn(
            line,
            code::IGNORED_ANALYSIS,
            "`.step` is not supported: the deck is simulated once with the unstepped values",
        ),
        "meas" | "measure" | "four" | "fourier" => sink.warn(
            line,
            code::IGNORED_ANALYSIS,
            format!("`{card}` post-processing is not supported and was skipped"),
        ),
        "options" | "option" | "opt" => audit_options(toks, params, line, sink),
        "temp" => audit_temp(toks, params, line, sink),
        "nodeset" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            "`.nodeset` is not applied: no initial guess is used (bistable circuits may settle in another state)",
        ),
        "global" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!(
                "`.global {}` is not supported: such nodes stay local inside subcircuits",
                toks[1..].join(" ")
            ),
        ),
        "include" | "inc" | "lib" | "hdl" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!(
                "`{}` is not supported: the file is not read, so its models, subcircuits and cards are missing",
                toks.join(" ")
            ),
        ),
        "if" | "elseif" | "else" | "endif" => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!("`{card}`: conditional blocks are not evaluated, the elements of every branch are instantiated"),
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
        _ => sink.warn(
            line,
            code::IGNORED_DIRECTIVE,
            format!("unknown card `{card}` was ignored (SPICE rejects unknown cards)"),
        ),
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
            "temp" | "tnom" => {
                let at_default = parse_spice_number(v, params)
                    .map(|t| (t - 27.0).abs() < 1e-9)
                    .unwrap_or(false);
                if !at_default {
                    sink.warn(
                        line,
                        code::IGNORED_TEMPERATURE,
                        format!("`.options {tok}` is ignored: RustSpice always simulates at 300.15 K (27 C)"),
                    );
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
                "`.options {}` is ignored: RustSpice runs with fixed built-in settings (reltol 1e-6, vntol 1e-10, abstol 1e-12, gmin 1e-12, 300.15 K)",
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

fn audit_temp(toks: &[&str], params: &FxHashMap<String, f64>, line: usize, sink: &mut DiagSink) {
    let all_default = toks.len() > 1
        && toks[1..].iter().all(|t| {
            parse_spice_number(t, params)
                .map(|v| (v - 27.0).abs() < 1e-9)
                .unwrap_or(false)
        });
    if !all_default {
        sink.warn(
            line,
            code::IGNORED_TEMPERATURE,
            format!(
                "`{}` is ignored: RustSpice always simulates at 300.15 K (27 C)",
                toks.join(" ")
            ),
        );
    }
}

/// `.tran <tstep> <tstop> [tstart [tmax]] [uic]`: the reader parses tstart/tmax but the transient engine uses
/// neither, and `uic` is not stored at all.
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
                if v != 0.0 {
                    sink.warn(
                        line,
                        code::IGNORED_TRAN_OPTION,
                        format!("`.tran` tstart={t} is ignored: results are produced from t=0"),
                    );
                }
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
        audit_number(t, params, ".tran", "tmax", line, sink);
        sink.warn(
            line,
            code::IGNORED_TRAN_OPTION,
            format!("`.tran` tmax={t} is ignored: RustSpice uses the fixed step tstep (no step limit, no breakpoints)"),
        );
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

/// `.dc <src> <start> <stop> <step> [<src2> ...]`: only the first sweep is read.
pub(crate) fn audit_dc(toks: &[&str], line: usize, sink: &mut DiagSink) {
    if toks.len() > 5 {
        sink.warn(
            line,
            code::DC_SWEEP,
            format!(
                "`.dc` fields `{}` were ignored: nested / additional sweeps are not supported, only `{}` is swept",
                toks[5..].join(" "),
                toks[1]
            ),
        );
    }
}

/// `.ic`: only `v(node)=value` tokens without blanks around `=` are understood.
pub(crate) fn audit_ic(
    toks: &[&str],
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) {
    let mut rejected: Vec<&str> = Vec::new();
    let mut accepted = 0usize;
    for tok in &toks[1..] {
        let lower = tok.to_ascii_lowercase();
        let ok = lower
            .strip_prefix("v(")
            .and_then(|rest| rest.split_once(")="))
            .map(|(_, val)| parse_spice_number(val, params).is_some())
            .unwrap_or(false);
        if ok {
            accepted += 1;
        } else {
            rejected.push(tok);
        }
    }
    if !rejected.is_empty() {
        sink.warn(
            line,
            code::IGNORED_IC,
            format!(
                "`.ic` tokens `{}` were not understood and ignored (only `v(node)=value` without blanks around `=` is read)",
                rejected.join(" ")
            ),
        );
    }
    if accepted > 0 {
        sink.note(
            line,
            code::IGNORED_IC,
            "`.ic` values are imposed after the t=0 operating point is solved (SPICE solves that point with the .ic nodes forced); unlisted nodes keep their operating-point values",
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
    let honored: &[&str] = if mtype == "D" {
        &["is", "n", "cjo"]
    } else {
        &[
            "level", "vto", "vt0", "kp", "gamma", "phi", "lambda", "cgso", "cgdo", "cgbo", "w", "l",
        ]
    };
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
            sink.warn(
                line,
                code::MALFORMED_INPUT,
                format!("model `{model}`: value `{shown}` of parameter `{}` is not a number; the parameter was dropped and its default is used", p.key.to_ascii_uppercase()),
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
        if k == "tnom" {
            sink.warn(
                line,
                code::IGNORED_TEMPERATURE,
                format!(
                    "model `{model}`: TNOM={} is ignored (fixed 300.15 K, no temperature scaling)",
                    p.raw
                ),
            );
        } else if k == "uo" && mtype != "D" && pairs.iter().any(|q| q.key == "kp") {
            // KP wins over UO in the reader: UO is simply not used
            unmodelled.push(up);
        } else if k == "uo" && mtype != "D" {
            sink.warn(
                line,
                code::IGNORED_MODEL_PARAM,
                format!("model `{model}`: UO={} is read as the transconductance KP (A/V^2); SPICE derives KP = UO*Cox from TOX", p.raw),
            );
        } else if k == "cjo" && mtype == "D" {
            if p.val.unwrap_or(0.0) > 0.0 {
                sink.warn(
                    line,
                    code::IGNORED_MODEL_PARAM,
                    format!("model `{model}`: CJO={} is applied as a constant capacitance; its bias dependence (VJ, M, FC) is not modelled", p.raw),
                );
            }
        } else if k == "cgbo" && mtype != "D" {
            if p.val.unwrap_or(0.0) > 0.0 {
                sink.warn(
                    line,
                    code::IGNORED_MODEL_PARAM,
                    format!("model `{model}`: CGBO={} is multiplied by the channel width W (SPICE scales it with the channel length)", p.raw),
                );
            }
        } else if k == "level" && mtype != "D" {
            if p.val.unwrap_or(1.0) >= 2.0 {
                sink.warn(
                    line,
                    code::WRONG_MODEL_LEVEL,
                    format!("model `{model}`: LEVEL={} is not implemented; the device is evaluated with the EKV model, not with the requested model", p.raw),
                );
            }
        } else if k == "phi" && mtype != "D" {
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
}

impl Elem {
    fn honored(self) -> &'static [&'static str] {
        match self {
            Elem::R => &["r"],
            Elem::C | Elem::L => &[],
            Elem::G => &[],
            Elem::D => &["area"],
            Elem::M => &["w", "l"],
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
            Elem::M => &[
                "ad",
                "as",
                "pd",
                "ps",
                "nrd",
                "nrs",
                "off",
                "geo",
                "delvto",
                "sens",
                "dtemp_off",
            ],
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
                    sink.warn(
                        line,
                        code::MALFORMED_INPUT,
                        format!("{name}: value `{v}` of `{k}` is not a number and was dropped"),
                    );
                } else {
                    audit_number(v, params, name, k, line, sink);
                }
            } else if k.eq_ignore_ascii_case("m") {
                sink.warn(
                    line,
                    code::IGNORED_MULTIPLIER,
                    format!("{name}: `{tok}` multiplier is ignored (the device is not replicated)"),
                );
            } else if key_in(k, &["temp", "dtemp", "tc1", "tc2", "tce"]) {
                sink.warn(
                    line,
                    code::IGNORED_TEMPERATURE,
                    format!("{name}: `{tok}` is ignored: RustSpice simulates at a fixed 300.15 K with no temperature coefficients"),
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
                sink.warn(
                    line,
                    code::MALFORMED_INPUT,
                    format!("{name}: unknown parameter `{k}` was ignored (SPICE rejects unknown instance parameters)"),
                );
            }
        } else {
            positional += 1;
            if kind == Elem::D && positional == 1 && num_value(tok, params).is_some() {
                sink.warn(
                    line,
                    code::IGNORED_MULTIPLIER,
                    format!("{name}: positional area `{tok}` is ignored (use `area=`)"),
                );
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
        sink.warn(
            line,
            code::MALFORMED_INPUT,
            format!(
                "{name}: extra token(s) `{}` after the value were ignored",
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
            sink.warn(
                line,
                code::MISPARSE,
                format!("{name}: `{n}` is used as a node name; it looks like unsupported syntax (e.g. POLY(...)) or a missing field"),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// independent sources
// ---------------------------------------------------------------------------------------------------------

const FUNCS: &[(&str, &str)] = &[
    ("pulse", "PULSE"),
    ("sin", "SIN"),
    ("pwl", "PWL"),
    ("exp", "EXP"),
    ("sffm", "SFFM"),
    ("am", "AM"),
    ("trnoise", "TRNOISE"),
    ("trrandom", "TRRANDOM"),
    ("external", "EXTERNAL"),
];

/// Byte offset of `tok` inside `card` when `tok` is a sub-slice of it (tokens come from `split_whitespace`).
fn offset_in(card: &str, tok: &str) -> Option<usize> {
    let (c0, t0) = (card.as_ptr() as usize, tok.as_ptr() as usize);
    (t0 >= c0 && t0 + tok.len() <= c0 + card.len()).then(|| t0 - c0)
}

/// Audits the value / waveform specification (`spec` = tokens after the two nodes of `card`) of a V or I source
/// against the waveform the reader actually built (`built`). Returns the text of a dropped `DC <v>` attribute
/// (its severity depends on the analyses of the deck, which the caller knows at the end).
pub(crate) fn audit_source(
    name: &str,
    card: &str,
    spec: &[&str],
    built: &SourceWaveform,
    params: &FxHashMap<String, f64>,
    line: usize,
    sink: &mut DiagSink,
) -> Option<String> {
    // fast path: a single clean literal (`V1 a 0 5`)
    if spec.is_empty() || (spec.len() == 1 && is_clean_literal(spec[0])) {
        return None;
    }
    let built_kind = match built {
        SourceWaveform::Dc(_) => "dc",
        SourceWaveform::Pulse { .. } => "pulse",
        SourceWaveform::Sin { .. } => "sin",
    };
    // intended waveform function: first token that starts with a known function name
    let func: Option<(usize, &'static str, &'static str)> =
        spec.iter().enumerate().find_map(|(i, t)| {
            let head = t.split('(').next().unwrap_or("");
            FUNCS
                .iter()
                .find(|f| head.eq_ignore_ascii_case(f.0))
                .map(|f| (i, f.0, f.1))
        });
    let track = spec.len() <= 64;
    let mut consumed: u64 = 0;
    let mut mark = |i: usize| {
        if i < 64 {
            consumed |= 1u64 << i;
        }
    };
    let dc_val = built.dc_value();

    // 1) leading plain value
    let first = spec[0];
    let is_kw = first.eq_ignore_ascii_case("dc")
        || first.eq_ignore_ascii_case("ac")
        || func.map(|f| f.0 == 0).unwrap_or(false);
    if !is_kw {
        if num_value(first, params).is_some() {
            mark(0);
            audit_number(first, params, name, "value", line, sink);
        } else if first.len() >= 3 && first.as_bytes()[..3].eq_ignore_ascii_case(b"dc=") {
            mark(0);
            sink.warn(
                line,
                code::DC_VALUE_DROPPED,
                format!("{name}: `{first}` is not recognised (write `DC <value>`); the source value defaults to 0"),
            );
        } else {
            mark(0);
            sink.warn(
                line,
                code::MISPARSE,
                format!("{name}: value `{first}` is not a number; the source defaults to 0"),
            );
        }
    }

    // 2) `DC <v>` and `AC <mag> [phase]` attributes
    let mut explicit_dc: Option<(String, f64)> = None;
    let mut i = 0;
    while i < spec.len() {
        if spec[i].eq_ignore_ascii_case("dc") {
            mark(i);
            if let Some(v) = spec.get(i + 1) {
                mark(i + 1);
                match num_value(v, params) {
                    Some(x) => {
                        audit_number(v, params, name, "DC value", line, sink);
                        explicit_dc = Some(((*v).to_string(), x));
                    }
                    None => sink.warn(
                        line,
                        code::MISPARSE,
                        format!("{name}: DC value `{v}` is not a number and was dropped"),
                    ),
                }
            }
        } else if spec[i].eq_ignore_ascii_case("ac") {
            mark(i);
            for j in 1..=2 {
                if let Some(v) = spec.get(i + j) {
                    if num_value(v, params).is_some() {
                        mark(i + j);
                    }
                }
            }
        }
        i += 1;
    }

    // 3) waveform function
    let Some((fi, fname, upper_name)) = func else {
        // no function at all: did the keyword search still fire on the element name?
        if built_kind != "dc" {
            sink.warn(
                line,
                code::MISPARSE,
                format!("{name}: a {} waveform was built from a keyword found in the element name, not from the source specification", built_kind.to_ascii_uppercase()),
            );
        }
        if track {
            report_leftovers(name, spec, consumed, line, sink);
        }
        return None;
    };
    // text of the card from the function keyword on (no allocation when the token is a slice of the card)
    let joined_fallback: String;
    let rest: &str = match offset_in(card, spec[fi]) {
        Some(off) => &card[off..],
        None => {
            joined_fallback = spec[fi..].join(" ");
            &joined_fallback
        }
    };
    let open = rest.find('(');
    let close = rest.find(')');
    let (args_text, balanced): (&str, bool) = match (open, close) {
        (Some(l), Some(r)) if l < r => (&rest[l + 1..r], true),
        (Some(l), _) => (&rest[l + 1..], false),
        (None, _) => (&rest[fname.len()..], false),
    };
    // tokens consumed by the function span
    let mut end_tok = spec.len() - 1;
    if balanced {
        if let Some(j) = (fi..spec.len()).find(|&j| spec[j].contains(')')) {
            end_tok = j;
        }
    }
    for j in fi..=end_tok {
        mark(j);
    }

    if matches!(
        fname,
        "pwl" | "exp" | "sffm" | "am" | "trnoise" | "trrandom" | "external"
    ) {
        sink.warn(
            line,
            code::IGNORED_WAVEFORM,
            format!("{name}: `{upper_name}(...)` waveforms are not supported; the source is a constant {dc_val} (SPICE evaluates the waveform)"),
        );
        return None;
    }
    let min_args = if fname == "pulse" { 2 } else { 3 };

    // argument scan: counts, the first 8 values, the unparsable ones
    let mut vals = [0.0f64; 8];
    let mut n_ok = 0usize;
    let mut dropped: Vec<&str> = Vec::new();
    for tok in args_text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
    {
        match num_value(tok, params) {
            Some(v) => {
                if n_ok < vals.len() {
                    vals[n_ok] = v;
                }
                n_ok += 1;
                if !is_clean_literal(tok) {
                    audit_number(tok, params, name, upper_name, line, sink);
                }
            }
            None => dropped.push(tok),
        }
    }

    if built_kind == "dc" || built_kind != fname {
        let reason = if open.is_none() {
            format!("`{upper_name}` is not followed by a parenthesised argument list")
        } else if !balanced {
            "the argument list is not closed by `)`".to_string()
        } else if n_ok < min_args {
            format!("{n_ok} usable numeric arguments found, {min_args} required")
        } else {
            format!(
                "the card was read as a {} waveform: the keyword is searched in the whole card, including the element name `{name}`",
                built_kind.to_ascii_uppercase()
            )
        };
        sink.warn(
            line,
            code::IGNORED_WAVEFORM,
            format!("{name}: {upper_name} waveform not built ({reason}); the source is evaluated as the constant {dc_val}"),
        );
        if !dropped.is_empty() {
            sink.warn(
                line,
                code::EXPR_TRUNCATION,
                format!(
                    "{name}: argument(s) `{}` could not be parsed as numbers and were dropped",
                    dropped.join(" ")
                ),
            );
        }
        return None;
    }

    // built as intended: audit the arguments
    if !dropped.is_empty() {
        sink.warn(
            line,
            code::EXPR_TRUNCATION,
            format!("{name}: {upper_name} argument(s) `{}` could not be parsed and were dropped; the later arguments shift left", dropped.join(" ")),
        );
    }
    if fname == "pulse" {
        if n_ok < 5 {
            let missing = if n_ok < 4 {
                "rise and fall times"
            } else {
                "fall time"
            };
            sink.warn(
                line,
                code::MISPARSE,
                format!("{name}: PULSE has {n_ok} arguments; the {missing} default(s) to 0 (SPICE: the .tran step)"),
            );
        }
        if n_ok > 7 {
            sink.warn(
                line,
                code::IGNORED_WAVEFORM,
                format!(
                    "{name}: {} PULSE argument(s) beyond the 7th were ignored",
                    n_ok - 7
                ),
            );
        }
    } else {
        if n_ok > 5 && vals[5] != 0.0 {
            sink.warn(
                line,
                code::IGNORED_WAVEFORM,
                format!(
                    "{name}: SIN phase argument {} was ignored (the sine starts at phase 0)",
                    vals[5]
                ),
            );
        }
        if n_ok > 6 {
            sink.warn(
                line,
                code::IGNORED_WAVEFORM,
                format!("{name}: SIN arguments beyond the 6th were ignored"),
            );
        }
    }
    if track {
        report_leftovers(name, spec, consumed, line, sink);
    }
    // `DC <v>` next to a waveform only matters for .op/.dc: the caller decides the severity once it knows the analyses
    explicit_dc.filter(|(_, v)| *v != dc_val).map(|(raw, v)| {
        format!("{name}: `DC {raw}` is discarded: .op/.dc use the waveform's initial value {dc_val} (SPICE uses the DC value {v})")
    })
}

fn report_leftovers(name: &str, spec: &[&str], consumed: u64, line: usize, sink: &mut DiagSink) {
    let left: Vec<&str> = spec
        .iter()
        .enumerate()
        .filter(|(i, _)| consumed & (1u64 << i) == 0)
        .map(|(_, t)| *t)
        .collect();
    if !left.is_empty() {
        sink.warn(
            line,
            code::MALFORMED_INPUT,
            format!(
                "{name}: unrecognised source field(s) `{}` were ignored",
                left.join(" ")
            ),
        );
    }
}

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
            (".options gmin=1e-9\n.op", "ignored-option"),
            (".nodeset v(out)=1\n.op", "ignored-directive"),
            (".include models.lib\n.op", "ignored-directive"),
            (".frob 1 2\n.op", "ignored-directive"),
            (".ac dec 10 1 1k", "ignored-analysis"),
            (".temp 85\n.op", "ignored-temperature"),
            (".tran 1n 1u 0 10n", "ignored-tran-option"),
            (".tran 1n 1u uic", "ignored-ic"),
            (".ic V(out) = 1\n.tran 1n 1u", "ignored-ic"),
            ("R3 out 0 1k m=2\n.op", "ignored-multiplier"),
            ("R3 out 0 1k tc1=0.1\n.op", "ignored-temperature"),
            ("R3 out 0 {2*rv}\n.param rv=1k\n.op", "expr-truncation"),
            ("R3 out 0 1x\n.op", "misparse"),
            ("R3 out 0 1k\nR3 out 0 1k\n.op", "malformed-input"),
            ("D9 out 0 NOPE\n.op", "undefined-model"),
            (
                ".model MX NMOS(LEVEL=3 VTO=1)\nM1 out in 0 0 MX W=1u L=1u\n.op",
                "wrong-model-level",
            ),
            (
                ".model DM D(RS=10 BV=5)\nD9 out 0 DM\n.op",
                "ignored-model-param",
            ),
            ("V9 a 0 PWL(0 0 1u 1)\n.op", "ignored-waveform"),
            ("V9 a 0 PULSE(0 1 0 1n 1n 1u\n.op", "ignored-waveform"),
            ("V9 a 0 SIN(0 1 1k 0 0 90)\n.op", "ignored-waveform"),
            ("V9 a 0 DC 2 SIN(1 1 1k)\n.op", "dc-value-dropped"),
            (".subckt foo a b\nR9 a b 1k\n.op", "malformed-input"),
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
    fn harmless_forms_stay_silent() {
        let base = "t\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 1k\n";
        for extra in [
            ".temp 27\n.op",
            ".options temp=27\n.op",
            "R3 out 0 1k5\n.op",
            "C1 out 0 10uF\n.op",
            "V9 a 0 PULSE(0 1 0 1n 1n 1u 2u)\n.op",
            "V9 a 0 SIN(0 1 1k 0 0 0)\n.op",
            "V9 a 0 DC 0 PULSE(0 1 0 1n 1n 1u 2u)\n.op",
            ".param k=3\nR3 out 0 {k}\n.op",
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
        // the legacy (lenient) readings are preserved bit for bit: only a diagnostic is added
        let n = parse_netlist(
            "t\n.param rv=1k\nV1 in 0 PWL(0 0 1u 1)\nR1 in out {2*rv}\nR2 out 0 1x\n.op\n",
        )
        .unwrap();
        assert_eq!(n.resistors[0].resistance, 2.0);
        assert_eq!(n.resistors[1].resistance, 1e6);
        assert_eq!(n.v_sources[0].waveform.dc_value(), 0.0);
        assert!(n.diagnostics.len() >= 3);
    }

    #[test]
    fn diagnostics_are_bounded() {
        let mut deck = String::from("t\nV1 in 0 1\n");
        for i in 0..1000 {
            deck.push_str(&format!("R{i} in 0 1k m=2\n"));
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
                .map_or(false, |b| b.is_ascii_digit() || *b == b'.')
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
        assert!(has("\nV1 in 0 1\nR1 in 0 1k\n.op\n", "title-line"));
        assert!(has(
            ".subckt x a b\nV1 1 0 5\nR1 1 0 1k\n.op\n",
            "title-line"
        ));
        assert!(!has("title\nV1 in 0 1\nR1 in 0 1k\n.op\n", "title-line"));
    }
}
