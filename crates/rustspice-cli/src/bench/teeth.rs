//! `--teeth`: mutation-sensitivity check of the comparison itself (SUITE_SPEC 14.3).
//!
//! A `pass` only means "RustSpice agrees with ngspice on the probes, within the tolerance". If the circuit were
//! slightly different the case could still pass when the probes do not depend on the element that changed, or the
//! tolerance is looser than the effect, or the probes are vacuous. The check therefore re-runs RustSpice alone on
//! copies of the deck in which up to three value-carrying elements (the first, a middle and the last one) are scaled
//! by [`FACTOR`] and compares each result against the ORIGINAL ngspice data with the case's own tolerances. A pass
//! whose every mutation still passes is `blunt`: the comparison cannot see the circuit.
//!
//! This file holds the pure parts: which tokens of a deck can be scaled ([`scan`]), the text edit ([`apply`]), the
//! pick rule ([`pick`]), the per-case verdict ([`verdict`]) and the report types. Running the mutants lives in
//! `mod.rs` next to the other protocol steps.
//!
//! What counts as a "value-carrying element" (and what does not):
//! * `R C L` value (4th token), `G` gain (6th token), `M` `W=` parameter, `V I` DC value (bare or after `DC`) or, in
//!   a transient, the first non-zero level of the waveform (`PULSE`/`SIN`/`EXP`: first non-zero of the first two
//!   parameters; `PWL`: first non-zero value);
//! * only whole, plain numbers: values given through `{expr}`, `'expr'`, a parameter name, `R=`/`value=` forms,
//!   zero values (a scaled 0 is 0), cards followed by `+` continuation lines and everything inside `.subckt` /
//!   `.control` blocks are skipped (they are the "`.param`/subckt value indirection and non-simple cards" of the
//!   spec);
//! * physics filter (deviation from the literal spec text, documented here and in the README): `C` and `L` values
//!   cannot change a DC `.op`/`.dc` result, and the swept source of a `.dc` card is overwritten by the sweep, so
//!   those elements are not counted for decks whose analyses make them irrelevant; scaling them would report
//!   "blunt" for a property of the circuit, not of the comparison.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use super::meta::parse_spice_number;

/// Scale applied to the mutated value (5 %).
pub const FACTOR: f64 = 1.05;

/// A response of the probes below this (relative, or normalised to the swing for transients) is "no dependence":
/// far above the 1e-12 noise of a solve, far below the effect of any element that sits in the signal path.
pub const INDEPENDENT_BELOW: f64 = 1e-7;

/// One scalable number of a deck.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// 0-based line index.
    pub line: usize,
    /// Element name as written (`R1`).
    pub name: String,
    /// Upper-case first letter: `R C L V I G M`.
    pub kind: char,
    /// What is scaled: `value`, `dc`, `W`, `pulse.v1`, `sin.vo`, `pwl.v2`, ...
    pub param: String,
    /// Byte range of the number in the deck text.
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Scan {
    /// In deck order.
    pub candidates: Vec<Candidate>,
    /// Why element cards were not candidates: reason -> number of cards.
    pub skipped: BTreeMap<String, usize>,
}

impl Scan {
    fn skip(&mut self, why: &str) {
        *self.skipped.entry(why.to_string()).or_default() += 1;
    }
    /// `3 inside_subckt, 2 expression_or_param_value` (empty when nothing was skipped).
    pub fn skipped_summary(&self) -> String {
        self.skipped
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Text before an inline comment (`;` anywhere, `$` when it starts a token).
fn strip_comment(line: &str) -> &str {
    let mut cut = line.len();
    if let Some(i) = line.find(';') {
        cut = cut.min(i);
    }
    let b = line.as_bytes();
    for i in 0..line.len() {
        if b[i] == b'$' && (i == 0 || b[i - 1].is_ascii_whitespace()) {
            cut = cut.min(i);
            break;
        }
    }
    &line[..cut]
}

/// Byte ranges of the tokens of `s` (separators: whitespace; with `parens` also `(`, `)` and `,`).
fn split_tokens(s: &str, parens: bool) -> Vec<(usize, usize)> {
    let sep = |c: u8| c.is_ascii_whitespace() || (parens && matches!(c, b'(' | b')' | b','));
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && sep(b[i]) {
            i += 1;
        }
        let st = i;
        while i < b.len() && !sep(b[i]) {
            i += 1;
        }
        if i > st {
            out.push((st, i));
        }
    }
    out
}

/// A whole plain number (`5`, `1k`, `-2.5e-3`, `10meg`), no expression and no `name=value`.
fn plain_number(t: &str) -> Option<f64> {
    let first = t.chars().next()?;
    let digit_start = first.is_ascii_digit()
        || (matches!(first, '.' | '+' | '-')
            && t.chars()
                .nth(1)
                .is_some_and(|c| c.is_ascii_digit() || c == '.'));
    if !digit_start || t.contains(['=', '{', '}', '\'', '"', '$']) {
        return None;
    }
    parse_spice_number(t).filter(|v| v.is_finite())
}

fn looks_like_expression(t: &str) -> bool {
    t.contains(['{', '}', '\''])
}

/// Sources: which token to scale. `Err` carries the skip reason.
fn source_token(lt: &[String], has_tran: bool) -> Result<(usize, String), &'static str> {
    let n = lt.len();
    if n < 4 {
        return Err("unsupported_form");
    }
    let nonzero = |k: usize| plain_number(&lt[k]).is_some_and(|v| v != 0.0);
    let dc: Option<usize> = if plain_number(&lt[3]).is_some() {
        Some(3)
    } else if lt[3] == "dc" && n > 4 && plain_number(&lt[4]).is_some() {
        Some(4)
    } else {
        None
    };
    let kw_at = (3..n).find(|&k| {
        matches!(
            lt[k].as_str(),
            "pulse" | "sin" | "pwl" | "exp" | "sffm" | "am" | "trnoise" | "trrandom"
        )
    });
    // numeric parameters following the keyword
    let (kw, params): (&str, Vec<usize>) = match kw_at {
        Some(k) => (
            lt[k].as_str(),
            (k + 1..n)
                .take_while(|&j| plain_number(&lt[j]).is_some())
                .collect(),
        ),
        None => ("", Vec::new()),
    };
    if matches!(kw, "sffm" | "am" | "trnoise" | "trrandom") {
        return Err("unsupported_source_form");
    }
    // the transient level: first non-zero of the amplitude-carrying parameters
    let wave_level = |kw: &str, p: &[usize]| -> Option<(usize, String)> {
        match kw {
            "pulse" | "exp" | "sin" => {
                let names: [&str; 2] = if kw == "sin" {
                    ["vo", "va"]
                } else {
                    ["v1", "v2"]
                };
                p.iter()
                    .take(2)
                    .enumerate()
                    .find(|(_, &j)| nonzero(j))
                    .map(|(pos, &j)| (j, format!("{kw}.{}", names[pos])))
            }
            "pwl" => p
                .iter()
                .enumerate()
                .skip(1)
                .step_by(2)
                .find(|(_, &j)| nonzero(j))
                .map(|(pos, &j)| (j, format!("pwl.v{}", pos / 2 + 1))),
            _ => None,
        }
    };
    // the level a DC `.op`/`.dc` sees: the value at t = 0
    let op_level = |kw: &str, p: &[usize]| -> Option<(usize, String)> {
        let (idx, name) = match kw {
            "pulse" | "exp" => (0, format!("{kw}.v1")),
            "sin" => (0, "sin.vo".to_string()),
            "pwl" => (1, "pwl.v1".to_string()),
            _ => return None,
        };
        p.get(idx)
            .copied()
            .filter(|&j| nonzero(j))
            .map(|j| (j, name))
    };
    let chosen = if has_tran {
        wave_level(kw, &params).or_else(|| {
            if kw.is_empty() {
                dc.filter(|&k| nonzero(k)).map(|k| (k, "dc".to_string()))
            } else {
                None
            }
        })
    } else {
        dc.filter(|&k| nonzero(k))
            .map(|k| (k, "dc".to_string()))
            .or_else(|| {
                if dc.is_none() {
                    op_level(kw, &params)
                } else {
                    None
                }
            })
    };
    chosen.ok_or_else(|| {
        if dc.is_none() && kw.is_empty() {
            "unsupported_form"
        } else {
            "no_nonzero_level"
        }
    })
}

/// Collect the scalable numbers of `deck`. `has_tran`: the deck runs a transient (C/L values matter).
pub fn scan(deck: &str, has_tran: bool) -> Scan {
    // lines with their byte offsets
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut off = 0;
    for l in deck.split_inclusive('\n') {
        lines.push((off, l));
        off += l.len();
    }
    let mut out = Scan::default();
    let mut in_control = false;
    let mut depth = 0usize;
    let mut swept: HashSet<String> = HashSet::new();
    // first pass: swept sources (the card may come after the elements)
    for (i, (_, l)) in lines.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let body = strip_comment(l).trim();
        let low = body.to_ascii_lowercase();
        if low == ".dc" || low.starts_with(".dc ") || low.starts_with(".dc\t") {
            for w in low.split_whitespace().skip(1) {
                swept.insert(w.to_string());
            }
        }
    }
    for (i, (off, l)) in lines.iter().enumerate() {
        if i == 0 {
            continue; // the title line is never a card
        }
        let body = strip_comment(l);
        let t = body.trim_start();
        if t.is_empty() || t.starts_with('*') || t.starts_with('+') {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if low.starts_with('.') {
            let card = low.split_whitespace().next().unwrap_or("");
            match card {
                ".control" => in_control = true,
                ".endc" => in_control = false,
                ".subckt" => depth += 1,
                ".ends" => depth = depth.saturating_sub(1),
                _ => {}
            }
            continue;
        }
        if in_control {
            continue;
        }
        let kind = t.chars().next().unwrap().to_ascii_uppercase();
        if !matches!(kind, 'R' | 'C' | 'L' | 'V' | 'I' | 'G' | 'M') {
            continue;
        }
        if depth > 0 {
            out.skip("inside_subckt");
            continue;
        }
        if lines
            .get(i + 1)
            .is_some_and(|(_, n)| strip_comment(n).trim_start().starts_with('+'))
        {
            out.skip("continuation_lines");
            continue;
        }
        let toks = split_tokens(body, matches!(kind, 'V' | 'I'));
        let text_of = |k: usize| &body[toks[k].0..toks[k].1];
        let name = text_of(0).to_string();
        let name_low = name.to_ascii_lowercase();
        // the physics filter: what cannot influence the analyses of this deck is not a mutation target
        if !has_tran && matches!(kind, 'C' | 'L') {
            out.skip("irrelevant_at_dc");
            continue;
        }
        if swept.contains(&name_low) {
            out.skip("swept_source");
            continue;
        }
        // (token index in `toks`, parameter label, byte range override start offset inside the token)
        let located: Result<(usize, String, usize), &'static str> = match kind {
            'R' | 'C' | 'L' => {
                if toks.len() < 4 {
                    Err("unsupported_form")
                } else if looks_like_expression(text_of(3)) {
                    Err("expression_or_param_value")
                } else if plain_number(text_of(3)).is_some() {
                    Ok((3, "value".to_string(), 0))
                } else {
                    Err("non_numeric_value")
                }
            }
            'G' => {
                if toks.len() < 6 {
                    Err("unsupported_form")
                } else if looks_like_expression(text_of(5)) {
                    Err("expression_or_param_value")
                } else if plain_number(text_of(5)).is_some() {
                    Ok((5, "gain".to_string(), 0))
                } else {
                    Err("non_numeric_value")
                }
            }
            'M' => {
                let w = (6..toks.len()).find(|&k| {
                    text_of(k)
                        .get(..2)
                        .is_some_and(|p| p.eq_ignore_ascii_case("w="))
                });
                match w {
                    None => Err("no_W_param"),
                    Some(k) => {
                        let v = &text_of(k)[2..];
                        if looks_like_expression(v) {
                            Err("expression_or_param_value")
                        } else if plain_number(v).is_some() {
                            Ok((k, "W".to_string(), 2))
                        } else {
                            Err("non_numeric_value")
                        }
                    }
                }
            }
            _ => {
                let lt: Vec<String> = (0..toks.len())
                    .map(|k| text_of(k).to_ascii_lowercase())
                    .collect();
                if lt.iter().any(|x| looks_like_expression(x)) {
                    Err("expression_or_param_value")
                } else {
                    source_token(&lt, has_tran).map(|(k, p)| (k, p, 0))
                }
            }
        };
        match located {
            Err(why) => out.skip(why),
            Ok((k, param, skip_prefix)) => {
                let s = toks[k].0 + skip_prefix;
                let e = toks[k].1;
                let text = body[s..e].to_string();
                match plain_number(&text) {
                    Some(v) if v != 0.0 && (v * FACTOR).is_finite() => {
                        out.candidates.push(Candidate {
                            line: i,
                            name,
                            kind,
                            param,
                            start: off + s,
                            end: off + e,
                            text,
                            value: v,
                        })
                    }
                    Some(_) => out.skip("zero_value"),
                    None => out.skip("non_numeric_value"),
                }
            }
        }
    }
    out
}

/// Indices of the first, a middle and the last candidate (distinct, ascending): up to three mutations.
pub fn pick(n: usize) -> Vec<usize> {
    let mut v: Vec<usize> = match n {
        0 => Vec::new(),
        _ => vec![0, n / 2, n - 1],
    };
    v.dedup();
    v
}

/// The new number text for `c`.
pub fn mutated_text(c: &Candidate, factor: f64) -> String {
    format!("{:.9e}", c.value * factor)
}

/// `deck` with the number of `c` scaled by `factor`; everything else is byte-identical.
pub fn apply(deck: &str, c: &Candidate, factor: f64) -> String {
    let mut s = String::with_capacity(deck.len() + 16);
    s.push_str(&deck[..c.start]);
    s.push_str(&mutated_text(c, factor));
    s.push_str(&deck[c.end..]);
    s
}

// ---------------------------------------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct MutationResult {
    pub element: String,
    /// 1-based line number in the deck.
    pub line: usize,
    pub kind: String,
    pub param: String,
    pub from: String,
    pub to: String,
    /// `detected` (the mutated run fails the comparison) | `survived` (still passes) | `unevaluable`.
    pub outcome: String,
    /// RustSpice on the mutated deck: `ok` | `loud_error` | `panic` | `timeout` | `unreadable_output`.
    pub rustspice: String,
    /// Worst `err / allowed` over the evaluated probes of (mutated RustSpice vs the original ngspice data):
    /// above 1 the comparison fails.
    pub worst_tol_ratio: Option<f64>,
    /// How far the mutation moved the probes: worst relative change (op/dc) or swing-normalised change (tran) of
    /// the mutated RustSpice output against the unmutated RustSpice output, over non-vacuous probes.
    pub response: Option<f64>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TeethResult {
    /// `ok` | `blunt` | `n/a` | `skipped`
    pub status: String,
    /// `blunt`: `probe_independent` | `tolerance_too_loose` | `vacuous_probes`; `n/a`: `not_a_pass` |
    /// `no_numeric_comparison`; `skipped`: `non_utf8_deck` | `no_mutable_element` | `mutated_runs_failed`.
    pub reason_class: Option<String>,
    pub reason: Option<String>,
    pub factor: f64,
    /// Worst `err / allowed` of the UNMUTATED comparison (how much of the tolerance the pass already uses); a
    /// mutation is only informative relative to this.
    pub baseline_tol_ratio: Option<f64>,
    /// Elements whose mutation the comparison detected.
    pub detected_by: Vec<String>,
    /// `ok`, but only source (V/I) mutations were detected while at least one passive / MOS mutation survived: the
    /// comparison sees the signal amplitude, not the circuit behind it.
    pub sources_only: bool,
    /// For `sources_only`: why the surviving passive / MOS mutations were not seen (`probe_independent` |
    /// `tolerance_too_loose` | `vacuous_probes`).
    pub weak_reason_class: Option<String>,
    pub mutations: Vec<MutationResult>,
    /// Where the mutated decks of a blunt case were kept.
    pub artifacts: Option<String>,
}

impl TeethResult {
    pub fn not_applicable(class: &str, reason: String) -> TeethResult {
        TeethResult {
            status: "n/a".to_string(),
            reason_class: Some(class.to_string()),
            reason: Some(reason),
            factor: FACTOR,
            baseline_tol_ratio: None,
            detected_by: Vec::new(),
            sources_only: false,
            weak_reason_class: None,
            mutations: Vec::new(),
            artifacts: None,
        }
    }
    pub fn skipped(class: &str, reason: String) -> TeethResult {
        TeethResult {
            status: "skipped".to_string(),
            ..TeethResult::not_applicable(class, reason)
        }
    }
}

/// Per-case verdict computed from the mutation outcomes.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub status: String,
    pub reason_class: Option<String>,
    pub reason: Option<String>,
    pub detected_by: Vec<String>,
    pub sources_only: bool,
    pub weak_reason_class: Option<String>,
}

/// Why surviving mutations were not seen: no probe moved (independent), the probes moved but stayed inside the
/// tolerance, or every probe is vacuous.
fn survivor_class(survived: &[&MutationResult]) -> (&'static str, String) {
    let responses: Vec<f64> = survived.iter().filter_map(|m| m.response).collect();
    if responses.is_empty() {
        (
            "vacuous_probes",
            "every probe is vacuous (reference below the floor) for the mutated runs".to_string(),
        )
    } else if responses.iter().all(|r| *r < INDEPENDENT_BELOW) {
        (
            "probe_independent",
            format!(
                "the probes do not depend on the mutated element(s): largest response {:.1e}",
                responses.iter().cloned().fold(0.0, f64::max)
            ),
        )
    } else {
        let worst = survived
            .iter()
            .filter_map(|m| m.worst_tol_ratio)
            .fold(0.0, f64::max);
        (
            "tolerance_too_loose",
            format!(
                "a 5% change moves the probes by up to {:.1e} but the closest mutation reaches only {:.2} of the tolerance",
                responses.iter().cloned().fold(0.0, f64::max),
                worst
            ),
        )
    }
}

/// `ok` when any mutation is detected; `blunt` when at least one was evaluated and every evaluated one survived;
/// `skipped` when no mutated run produced a verdict. An `ok` whose detections are all source mutations while a
/// passive / MOS mutation survived is flagged `sources_only` (informational).
pub fn verdict(muts: &[MutationResult]) -> Verdict {
    let detected: Vec<&MutationResult> = muts.iter().filter(|m| m.outcome == "detected").collect();
    let survived: Vec<&MutationResult> = muts.iter().filter(|m| m.outcome == "survived").collect();
    let unevaluable = muts.len() - detected.len() - survived.len();
    let is_source = |m: &&MutationResult| matches!(m.kind.as_str(), "V" | "I");
    if !detected.is_empty() {
        let passive_survivors: Vec<&MutationResult> =
            survived.iter().filter(|m| !is_source(m)).cloned().collect();
        let sources_only = detected.iter().all(is_source) && !passive_survivors.is_empty();
        let weak = sources_only.then(|| survivor_class(&passive_survivors).0.to_string());
        return Verdict {
            status: "ok".to_string(),
            reason_class: None,
            reason: Some(format!(
                "{} of {} mutation(s) are detected by the comparison{}",
                detected.len(),
                muts.len(),
                if sources_only {
                    " (only through a source: every passive / MOS mutation survived)"
                } else {
                    ""
                }
            )),
            detected_by: detected.iter().map(|m| m.element.clone()).collect(),
            sources_only,
            weak_reason_class: weak,
        };
    }
    if survived.is_empty() {
        return Verdict {
            status: "skipped".to_string(),
            reason_class: Some("mutated_runs_failed".to_string()),
            reason: Some(format!(
                "none of the {} mutated run(s) produced an evaluable comparison",
                muts.len()
            )),
            detected_by: Vec::new(),
            sources_only: false,
            weak_reason_class: None,
        };
    }
    let (class, why) = survivor_class(&survived);
    let extra = if unevaluable > 0 {
        format!("; {unevaluable} mutation(s) not evaluable")
    } else {
        String::new()
    };
    Verdict {
        status: "blunt".to_string(),
        reason_class: Some(class.to_string()),
        reason: Some(format!("{why}{extra}")),
        detected_by: Vec::new(),
        sources_only: false,
        weak_reason_class: None,
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BluntCase {
    pub id: String,
    pub group: String,
    pub reason_class: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TeethSummary {
    pub factor: f64,
    pub ok: usize,
    pub blunt: usize,
    pub n_a: usize,
    pub skipped: usize,
    /// Blunt cases by reason class, and skipped / n/a cases by their class.
    pub blunt_by_reason: BTreeMap<String, usize>,
    pub skipped_by_reason: BTreeMap<String, usize>,
    pub na_by_reason: BTreeMap<String, usize>,
    pub blunt_cases: Vec<BluntCase>,
    /// `ok` cases detected only through a source mutation (see [`TeethResult::sources_only`]), by the reason the
    /// passive / MOS mutations were not seen.
    pub ok_sources_only: usize,
    pub sources_only_by_reason: BTreeMap<String, usize>,
    pub sources_only_cases: Vec<BluntCase>,
}

impl TeethSummary {
    pub fn add(&mut self, id: &str, group: &str, t: &TeethResult) {
        let class = t.reason_class.clone().unwrap_or_default();
        match t.status.as_str() {
            "ok" => {
                self.ok += 1;
                if t.sources_only {
                    self.ok_sources_only += 1;
                    let class = t.weak_reason_class.clone().unwrap_or_default();
                    *self
                        .sources_only_by_reason
                        .entry(class.clone())
                        .or_default() += 1;
                    self.sources_only_cases.push(BluntCase {
                        id: id.to_string(),
                        group: group.to_string(),
                        reason_class: class,
                        reason: t.reason.clone().unwrap_or_default(),
                    });
                }
            }
            "blunt" => {
                self.blunt += 1;
                *self.blunt_by_reason.entry(class.clone()).or_default() += 1;
                self.blunt_cases.push(BluntCase {
                    id: id.to_string(),
                    group: group.to_string(),
                    reason_class: class,
                    reason: t.reason.clone().unwrap_or_default(),
                });
            }
            "skipped" => {
                self.skipped += 1;
                *self.skipped_by_reason.entry(class).or_default() += 1;
            }
            _ => {
                self.n_a += 1;
                *self.na_by_reason.entry(class).or_default() += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands(deck: &str, tran: bool) -> Vec<Candidate> {
        scan(deck, tran).candidates
    }

    fn mutate(deck: &str, c: &Candidate) -> String {
        apply(deck, c, FACTOR)
    }

    #[test]
    fn passives_are_scaled_in_place_and_nothing_else_changes() {
        let deck = "title R9 9\nV1 in 0 DC 5 ; src\nR1 in out 1k\nR2 out 0 4.7MEG $ load\nC1 out 0 10p\n.tran 1n 1u\n.end\n";
        let c = cands(deck, true);
        let names: Vec<&str> = c.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["V1", "R1", "R2", "C1"]);
        assert_eq!(c[0].param, "dc");
        assert_eq!(c[0].text, "5");
        let m = mutate(deck, &c[2]);
        assert!(m.contains("R2 out 0 4.935000000e6 $ load\n"), "{m}");
        // exactly one line differs and the line count is the same
        let diff: Vec<_> = deck
            .lines()
            .zip(m.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(diff.len(), 1);
        assert_eq!(deck.lines().count(), m.lines().count());
        // the replaced token round-trips through the SPICE number parser at +5 %
        let v = parse_spice_number("4.935000000e6").unwrap();
        assert!((v / 4.7e6 - 1.05).abs() < 1e-12);
        // the title line is never a card (it starts with an R here)
        assert!(c.iter().all(|x| x.line != 0));
    }

    #[test]
    fn dc_and_op_decks_do_not_count_c_and_l_or_the_swept_source() {
        let deck =
            "t\nV1 in 0 5\nR1 in out 1k\nC1 out 0 1n\nL1 out x 1u\nR2 x 0 1k\n.dc V1 0 5 1\n.end\n";
        let s = scan(deck, false);
        let names: Vec<&str> = s.candidates.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["R1", "R2"], "{:?}", s.skipped);
        assert_eq!(s.skipped.get("irrelevant_at_dc"), Some(&2));
        assert_eq!(s.skipped.get("swept_source"), Some(&1));
        // the same elements all count in a transient (the swept-source rule is about `.dc` only)
        let tran = deck.replace(".dc V1 0 5 1", ".tran 1n 1u");
        let names: Vec<String> = scan(&tran, true)
            .candidates
            .iter()
            .map(|x| x.name.clone())
            .collect();
        assert_eq!(names, ["V1", "R1", "C1", "L1", "R2"]);
    }

    #[test]
    fn source_forms_pick_the_level_that_matters() {
        let f = |card: &str, tran: bool| -> Option<(String, String)> {
            let deck = format!("t\n{card}\nR1 a 0 1k\n.end\n");
            let s = scan(&deck, tran);
            s.candidates
                .iter()
                .find(|c| c.name.starts_with(['V', 'I']))
                .map(|c| (c.param.clone(), c.text.clone()))
        };
        let p = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        // DC forms
        assert_eq!(f("V1 a 0 5", false), p("dc", "5"));
        assert_eq!(f("V1 a 0 DC 3.3", true), p("dc", "3.3"));
        assert_eq!(f("V1 a 0 dc 2 ac 1", false), p("dc", "2"));
        assert_eq!(f("I1 a 0 1m", false), p("dc", "1m"));
        // a transient scales the waveform, not the (ignored) DC value
        assert_eq!(
            f("V1 a 0 DC 1 PULSE(0 5 0 1n 1n 1u 2u)", true),
            p("pulse.v2", "5")
        );
        assert_eq!(
            f("V1 a 0 PULSE(1 5 0 1n 1n 1u 2u)", true),
            p("pulse.v1", "1")
        );
        assert_eq!(f("V1 a 0 SIN(0 1 1meg)", true), p("sin.va", "1"));
        assert_eq!(f("V1 a 0 sin 2.5 1 1meg", true), p("sin.vo", "2.5"));
        assert_eq!(f("V1 a 0 PWL(0 0 1n 3 2n 3)", true), p("pwl.v2", "3"));
        // an op/dc analysis sees the DC value; a waveform-only source its t = 0 level
        assert_eq!(
            f("V1 a 0 DC 4 PULSE(0 5 0 1n 1n 1u 2u)", false),
            p("dc", "4")
        );
        assert_eq!(
            f("V1 a 0 PULSE(2 5 0 1n 1n 1u 2u)", false),
            p("pulse.v1", "2")
        );
        assert_eq!(
            f("V1 a 0 PULSE(0 5 0 1n 1n 1u 2u)", false),
            None,
            "t=0 level is zero"
        );
        // zero DC sources (ammeters), AC-only sources and exotic waveforms are not candidates
        assert_eq!(f("V1 a 0 0", false), None);
        assert_eq!(f("V1 a 0 AC 1", false), None);
        assert_eq!(f("V1 a 0 SFFM(0 1 1k 5 100)", true), None);
        // an expression is never touched
        assert_eq!(f("V1 a 0 DC {vdd}", false), None);
    }

    #[test]
    fn mos_width_gain_and_cards_that_are_not_simple() {
        let deck = "t\nM1 d g 0 0 nm W=10u L=1u\nM2 d g 0 0 nm l=1u w=2.5u m=2\nM3 d g 0 0 nm W = 5u L=1u\nM4 d g 0 0 nm W={wn} L=1u\nG1 o 0 a 0 2m\nG2 o 0 a 0 {gm}\nR1 a b\n+ 1k\nR2 a 0 {rv}\nR3 a 0 rv\nR4 a 0 0\nX1 a b sub\nV1 a 0 5\n.subckt sub p q\nR5 p q 1k\n.ends\n.control\nR6 a 0 1k\n.endc\nR7 a 0 2k\n.op\n.end\n";
        let s = scan(deck, false);
        let got: Vec<(String, String, String)> = s
            .candidates
            .iter()
            .map(|c| (c.name.clone(), c.param.clone(), c.text.clone()))
            .collect();
        let want = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
        assert_eq!(
            got,
            vec![
                want("M1", "W", "10u"),
                want("M2", "W", "2.5u"),
                want("G1", "gain", "2m"),
                want("V1", "dc", "5"),
                want("R7", "value", "2k"),
            ],
            "{:?}",
            s.skipped
        );
        for (why, n) in [
            ("no_W_param", 1),
            ("expression_or_param_value", 3),
            ("continuation_lines", 1),
            ("non_numeric_value", 1),
            ("zero_value", 1),
            ("inside_subckt", 1),
        ] {
            assert_eq!(s.skipped.get(why), Some(&n), "{why}: {:?}", s.skipped);
        }
        // the width edit keeps the `W=` prefix and the rest of the card
        let m = mutate(deck, &s.candidates[0]);
        assert!(m.contains("M1 d g 0 0 nm W=1.050000000e-5 L=1u\n"), "{m}");
        let m = mutate(deck, &s.candidates[1]);
        assert!(
            m.contains("M2 d g 0 0 nm l=1u w=2.625000000e-6 m=2\n"),
            "{m}"
        );
    }

    #[test]
    fn crlf_decks_and_missing_final_newline() {
        let deck = "t\r\nV1 a 0 5\r\nR1 a 0 1k";
        let c = cands(deck, false);
        assert_eq!(c.len(), 2);
        let m = mutate(deck, &c[1]);
        assert_eq!(m, "t\r\nV1 a 0 5\r\nR1 a 0 1.050000000e3");
        assert_eq!(
            mutate(deck, &c[0]),
            "t\r\nV1 a 0 5.250000000e0\r\nR1 a 0 1k"
        );
    }

    #[test]
    fn negative_and_exponent_values() {
        let deck = "t\nV1 a 0 -2\nR1 a 0 1e3\nI1 a 0 -1.5m\n.op\n";
        let c = cands(deck, false);
        assert_eq!(c.len(), 3);
        assert!(mutate(deck, &c[0]).contains("V1 a 0 -2.100000000e0"));
        assert!(mutate(deck, &c[1]).contains("R1 a 0 1.050000000e3"));
        assert!(mutate(deck, &c[2]).contains("I1 a 0 -1.575000000e-3"));
    }

    #[test]
    fn first_middle_last_are_distinct() {
        assert_eq!(pick(0), Vec::<usize>::new());
        assert_eq!(pick(1), [0]);
        assert_eq!(pick(2), [0, 1]);
        assert_eq!(pick(3), [0, 1, 2]);
        assert_eq!(pick(4), [0, 2, 3]);
        assert_eq!(pick(10), [0, 5, 9]);
    }

    fn mr(outcome: &str, response: Option<f64>, ratio: Option<f64>) -> MutationResult {
        MutationResult {
            element: "R1".into(),
            line: 3,
            kind: "R".into(),
            param: "value".into(),
            from: "1k".into(),
            to: "1.05e3".into(),
            outcome: outcome.into(),
            rustspice: "ok".into(),
            worst_tol_ratio: ratio,
            response,
            note: None,
        }
    }

    fn mk(kind: &str, outcome: &str, response: Option<f64>, ratio: Option<f64>) -> MutationResult {
        MutationResult {
            kind: kind.into(),
            element: format!("{kind}1"),
            ..mr(outcome, response, ratio)
        }
    }

    #[test]
    fn verdicts_and_blunt_reasons() {
        let v = verdict(&[
            mr("survived", Some(1e-3), Some(0.2)),
            mr("detected", Some(0.05), Some(50.0)),
        ]);
        assert_eq!((v.status.as_str(), v.reason_class.clone()), ("ok", None));
        assert_eq!(v.detected_by, ["R1"]);
        assert!(!v.sources_only);
        // all survive and the probes did move: the tolerance is the problem
        let v = verdict(&[
            mr("survived", Some(1e-3), Some(0.2)),
            mr("survived", Some(0.0), Some(0.0)),
        ]);
        assert_eq!(v.status, "blunt");
        assert_eq!(v.reason_class.as_deref(), Some("tolerance_too_loose"));
        assert!(v.reason.unwrap().contains("0.20 of the tolerance"));
        // all survive and nothing moved: the probes do not see those elements
        let v = verdict(&[
            mr("survived", Some(1e-13), Some(1e-9)),
            mr("survived", Some(0.0), Some(0.0)),
        ]);
        assert_eq!(
            (v.status.as_str(), v.reason_class.as_deref()),
            ("blunt", Some("probe_independent"))
        );
        // no response number at all: vacuous probes
        let v = verdict(&[mr("survived", None, None)]);
        assert_eq!(v.reason_class.as_deref(), Some("vacuous_probes"));
        // failed runs are neither detections nor survivals
        let v = verdict(&[mr("unevaluable", None, None), mr("unevaluable", None, None)]);
        assert_eq!(
            (v.status.as_str(), v.reason_class.as_deref()),
            ("skipped", Some("mutated_runs_failed"))
        );
        let v = verdict(&[
            mr("survived", Some(1e-3), Some(0.1)),
            mr("unevaluable", None, None),
        ]);
        assert_eq!(v.status, "blunt");
        assert!(v.reason.unwrap().contains("1 mutation(s) not evaluable"));
    }

    #[test]
    fn an_ok_that_only_a_source_mutation_detects_is_flagged() {
        // the amplitude is seen, the passives are not
        let v = verdict(&[
            mk("V", "detected", Some(0.05), Some(80.0)),
            mk("R", "survived", Some(0.0), Some(0.0)),
            mk("C", "survived", Some(1e-9), Some(0.01)),
        ]);
        assert_eq!(v.status, "ok");
        assert!(v.sources_only);
        assert_eq!(v.weak_reason_class.as_deref(), Some("probe_independent"));
        assert_eq!(v.detected_by, ["V1"]);
        let v = verdict(&[
            mk("I", "detected", Some(0.05), Some(80.0)),
            mk("R", "survived", Some(3e-3), Some(0.6)),
        ]);
        assert!(v.sources_only);
        assert_eq!(v.weak_reason_class.as_deref(), Some("tolerance_too_loose"));
        // a passive detection, or no passive mutated at all, is not "sources only"
        assert!(
            !verdict(&[
                mk("V", "detected", Some(0.05), Some(80.0)),
                mk("R", "detected", Some(0.05), Some(80.0))
            ])
            .sources_only
        );
        assert!(!verdict(&[mk("V", "detected", Some(0.05), Some(80.0))]).sources_only);
        assert!(
            !verdict(&[
                mk("R", "detected", Some(0.05), Some(80.0)),
                mk("V", "survived", Some(0.0), Some(0.0))
            ])
            .sources_only
        );
    }

    #[test]
    fn summary_counts_by_reason() {
        let mut s = TeethSummary::default();
        let t = |status: &str, class: &str| TeethResult {
            status: status.into(),
            reason_class: Some(class.into()),
            reason: Some("r".into()),
            factor: FACTOR,
            baseline_tol_ratio: None,
            detected_by: Vec::new(),
            sources_only: false,
            weak_reason_class: None,
            mutations: Vec::new(),
            artifacts: None,
        };
        s.add("a", "accuracy", &t("ok", ""));
        s.add("b", "accuracy", &t("blunt", "tolerance_too_loose"));
        s.add("c", "capability", &t("blunt", "probe_independent"));
        s.add("d", "capability", &t("skipped", "no_mutable_element"));
        s.add("e", "capability", &t("n/a", "not_a_pass"));
        assert_eq!((s.ok, s.blunt, s.skipped, s.n_a), (1, 2, 1, 1));
        assert_eq!(s.blunt_by_reason.get("probe_independent"), Some(&1));
        assert_eq!(s.blunt_cases.len(), 2);
        assert_eq!(s.skipped_by_reason.get("no_mutable_element"), Some(&1));
    }
}
