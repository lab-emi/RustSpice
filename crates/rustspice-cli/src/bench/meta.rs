//! `* bench.*` deck metadata (SUITE_SPEC section 2): strict parser, series expansion, ngspice deck rewriting.

use std::collections::HashSet;

/// Registered known-gap slugs (SUITE_SPEC section 11). A deck naming any other slug is rejected.
pub const KNOWN_GAP_SLUGS: &[&str] = &[
    "silent-ignore-directive",
    "silent-ignore-analysis",
    "silent-ignore-model-param",
    "silent-wrong-model-level",
    "silent-ignore-source-waveform",
    "silent-ignore-temperature",
    "silent-ignore-multiplier",
    "silent-ignore-ic",
    "expr-truncation",
    "dc-clobbers-waveform",
    "clamp-rail",
    "false-convergence-pnjlim",
    "stale-lu-after-faer-fallback",
    "no-op-homotopy-tran",
    "dc-sweep-no-recovery",
    "fixed-step-accuracy",
    "abstol-absolute-residual",
    "resource-budget",
    "symbolic-lu-blowup",
    "faer-nonlinear-slow",
    "r-nonpositive-clamp",
    "linear-static-pivot-accuracy",
    "batch-all-or-nothing",
    "adjoint-coverage",
    // phase 2 additions (SUITE_SPEC section 13.2)
    "unsupported-syntax-loud",
    "unsupported-device-loud",
    "lenient-malformed-input",
    "silent-misparse",
    "dc-value-ignored-with-waveform",
    "dc-static-homotopy",
    "exp-argument-clamp",
    "ill-posed-dc-silent",
    "title-line-parsed-as-card",
    // phase 2 calibration additions (SUITE_SPEC section 13.2, second batch)
    "tran-degenerate-last-step",
    "dc-sweep-grid-drift",
    "unrolled-lu-tran-divergence",
    // phase 3 additions (SUITE_SPEC section 15.1): mechanisms the second-look verification found under a wrong slug
    "nonfinite-result-accepted",
    "step-retry-grid-desync",
    "newton-iteration-cap",
    "param-floor-clamp",
];

/// Tags with engine semantics (documented in benchmarks/README.md). Every other tag is an informational label
/// that only groups and filters cases in the report.
///
/// * `ngspice_error_ok`: ngspice may print `singular matrix`/gmin messages and still exit 0 with a rawfile, so its
///   log patterns do not mark the run failed (a non-zero exit, a crash or a missing rawfile still do).
/// * `ngspice_unbounded`: ngspice is not run at all (it never terminates on this deck); `ngspice_status` is
///   `skipped_unbounded` and RustSpice is judged against `bench.expect` only.
/// * `npoints`: transient only; the RustSpice row count must equal the ngspice row count or
///   `floor(tstop/tstep + 1e-9) + 1` (+1 for a partial last step).
/// * `timing_edge`: transient only; report the maximum 50 %-crossing time shift.
/// * `ngspice_crash`: ngspice-47 is expected to crash (signal) on this deck; a note is added when it does not.
/// * `lenient_ok`: informational, with one engine effect: a comparison in which every probe is vacuous (reference
///   ~0) and RustSpice does not deviate is reported as `info` instead of `unverified`.
/// * `family:<id>`: generator family of the case.
pub const ENGINE_TAGS: &[&str] = &[
    "ngspice_error_ok",
    "ngspice_unbounded",
    "npoints",
    "timing_edge",
    "ngspice_crash",
    "lenient_ok",
];

/// Informational tags with an `ngspice_` prefix (documentation of what ngspice does on the deck). A new
/// `ngspice_*` tag that is in neither list is flagged by the corpus-inventory test.
pub const NGSPICE_NOTE_TAGS: &[&str] = &[
    "ngspice_warns",
    "ngspice_own_error",
    "ngspice_no_output",
    "ngspice_empty",
];

/// Informational behaviour flags of RustSpice, kept in the report.
/// `caught_panic_noise`: RustSpice prints a caught panic message and exits 1 (clean refusal, not a panic; a note is
/// added to every such case, tagged or not); `lenient_ok`: RustSpice accepts input that is malformed but harmless,
/// and ngspice does too (see [`ENGINE_TAGS`] for its one engine effect).
pub const INFO_BEHAVIOUR_TAGS: &[&str] = &["caught_panic_noise", "lenient_ok"];

const KNOWN_KEYS: &[&str] = &[
    "id",
    "group",
    "title",
    "probes",
    "tol.rel",
    "tol.abs",
    "tol.norm",
    "floor",
    "expect",
    "known_gap",
    "gap.note",
    "ng.options",
    "ng.solver",
    "ref",
    "ref.tmax",
    "oracle",
    "series.tstep",
    "size",
    "tags",
    "timing",
    "est_rss_mib",
    "note",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Capability,
    Accuracy,
    Robustness,
    Performance,
}

impl Group {
    pub fn parse(s: &str) -> Option<Group> {
        match s {
            "capability" => Some(Group::Capability),
            "accuracy" => Some(Group::Accuracy),
            "robustness" => Some(Group::Robustness),
            "performance" => Some(Group::Performance),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Group::Capability => "capability",
            Group::Accuracy => "accuracy",
            Group::Robustness => "robustness",
            Group::Performance => "performance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Match,
    LoudError,
    Either,
}

impl Expect {
    pub fn as_str(self) -> &'static str {
        match self {
            Expect::Match => "match",
            Expect::LoudError => "loud_error",
            Expect::Either => "either",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NgSolverSel {
    Sparse,
    Klu,
    Both,
}

/// Which ngspice solvers are timed/run as-is, in execution order (the first one is the accuracy reference solver).
impl NgSolverSel {
    pub fn solvers(self) -> &'static [NgSolver] {
        match self {
            NgSolverSel::Sparse => &[NgSolver::Sparse],
            NgSolverSel::Klu => &[NgSolver::Klu],
            NgSolverSel::Both => &[NgSolver::Sparse, NgSolver::Klu],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NgSolver {
    Sparse,
    Klu,
}

impl NgSolver {
    pub fn as_str(self) -> &'static str {
        match self {
            NgSolver::Sparse => "sparse",
            NgSolver::Klu => "klu",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RefMode {
    /// Default: tight options for op/dc, converged options plus tmax for tran.
    Default,
    /// Explicit `tight` (same options for tran, no automatic tmax change).
    Tight,
    /// No reference run: compare directly against the as-is run.
    None,
    /// Literal `.options` text.
    Options(String),
}

#[derive(Clone, Debug)]
pub struct Meta {
    pub id: String,
    pub group: Group,
    pub title: String,
    pub probes: Vec<String>,
    pub tol_rel: f64,
    pub tol_abs: f64,
    pub tol_norm: f64,
    pub floor: f64,
    pub expect: Expect,
    pub known_gap: Option<String>,
    pub gap_note: Option<String>,
    pub ng_options: Option<String>,
    pub ng_solver: NgSolverSel,
    pub reference: RefMode,
    pub ref_tmax: Option<f64>,
    pub oracle: Option<String>,
    pub series_tstep: Vec<String>,
    pub size: Option<u64>,
    pub tags: Vec<String>,
    pub timing: bool,
    pub est_rss_mib: Option<u64>,
    pub note: Option<String>,
}

pub const TIGHT_OPTIONS: &str = "reltol=1e-9 vntol=1e-12 abstol=1e-15";
pub const TRAN_REF_OPTIONS: &str = "reltol=1e-6 abstol=1e-15 vntol=1e-9";

impl Meta {
    pub fn has_tag(&self, t: &str) -> bool {
        self.tags.iter().any(|x| x == t)
    }
    /// ngspice may log failure-pattern lines and still produce a usable rawfile (tag `ngspice_error_ok`).
    pub fn ngspice_error_ok(&self) -> bool {
        self.has_tag("ngspice_error_ok")
    }
    /// ngspice runs away on this deck and is not started (tag `ngspice_unbounded`).
    pub fn ngspice_unbounded(&self) -> bool {
        self.has_tag("ngspice_unbounded")
    }
    /// `family:<id>` tag if present.
    pub fn family_tag(&self) -> Option<&str> {
        self.tags.iter().find_map(|t| t.strip_prefix("family:"))
    }
    pub fn defaults(id: &str) -> Meta {
        Meta {
            id: id.to_string(),
            group: Group::Capability,
            title: id.to_string(),
            probes: Vec::new(),
            tol_rel: 1e-6,
            tol_abs: 1e-9,
            tol_norm: 5e-3,
            floor: 1e-6,
            expect: Expect::Match,
            known_gap: None,
            gap_note: None,
            ng_options: None,
            ng_solver: NgSolverSel::Sparse,
            reference: RefMode::Default,
            ref_tmax: None,
            oracle: None,
            series_tstep: Vec::new(),
            size: None,
            tags: Vec::new(),
            timing: false,
            est_rss_mib: None,
            note: None,
        }
    }
}

/// Parsed metadata plus every diagnostic found (a case with diagnostics becomes `unverified`).
#[derive(Clone, Debug)]
pub struct ParsedMeta {
    pub meta: Meta,
    pub diags: Vec<String>,
    /// Non-blocking remarks (shown in the case notes).
    pub notes: Vec<String>,
}

/// SPICE number with engineering suffix (`10p`, `1meg`, `2u`, `1e-9`, `5nsec`). `None` if not a number.
pub fn parse_spice_number(tok: &str) -> Option<f64> {
    let t = tok.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let b = t.as_bytes();
    // longest numeric prefix accepted by f64::from_str, honouring exponent forms
    let mut end = 0;
    let mut seen_digit = false;
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
        if b[i].is_ascii_digit() {
            seen_digit = true;
        }
        i += 1;
    }
    if !seen_digit {
        return None;
    }
    end = end.max(i);
    if i < b.len() && b[i] == b'e' {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let ds = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds {
            end = j;
        }
    }
    let mant_str = &t[..end];
    let mant: f64 = mant_str.parse().ok()?;
    let suffix = &t[end..];
    // decimal exponent of the suffix (exact decimal scaling avoids 2.5*1e-6 != 2.5e-6)
    let exp10: i32 = if suffix.starts_with("meg") {
        6
    } else if suffix.starts_with("mil") {
        return Some(mant * 25.4e-6);
    } else {
        match suffix.chars().next() {
            None => 0,
            Some('t') => 12,
            Some('g') => 9,
            Some('k') => 3,
            Some('m') => -3,
            Some('u') => -6,
            Some('n') => -9,
            Some('p') => -12,
            Some('f') => -15,
            Some('a') => -18,
            Some(c) if c.is_ascii_alphabetic() => 0, // unit letters such as `v`, `s`, `a`... ignored by SPICE
            _ => return None,
        }
    };
    let v = if exp10 == 0 {
        mant
    } else if mant_str.contains('e') {
        mant * 10f64.powi(exp10)
    } else {
        format!("{mant_str}e{exp10}").parse().ok()?
    };
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

/// `[a-z0-9_-]+`: the shell-safe handle alphabet (the spec's `[a-z0-9_]+` plus `-`, which stems such as
/// `val_r_denormal_1e-400` need).
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
}

/// `v(<node>)` / `i(<source>)`, lower case. Node names may carry any printable character a schematic netlist
/// produces (`n+`, `bus[1]`, `a/b`, `x:y`), except parentheses, commas and blanks.
fn valid_probe(p: &str) -> bool {
    let ok_chars = |s: &str| {
        !s.is_empty()
            && s.bytes().all(|c| {
                c.is_ascii_graphic() && !c.is_ascii_uppercase() && !matches!(c, b'(' | b')' | b',')
            })
    };
    if let Some(inner) = p.strip_prefix("v(").or_else(|| p.strip_prefix("i(")) {
        if let Some(inner) = inner.strip_suffix(')') {
            return ok_chars(inner);
        }
    }
    false
}

/// The deck itself selects the KLU solver (`.options klu`): the solver check of the ngspice log must not
/// assume SPARSE then.
pub fn deck_selects_klu(deck: &str) -> bool {
    deck.lines().any(|l| {
        let t = l.trim_start().to_ascii_lowercase();
        (t.starts_with(".option") || t.starts_with(".opt"))
            && t.split_whitespace().skip(1).any(|w| w == "klu")
    })
}

/// `rest` (a comment line without its leading `*`) reads like a metadata line but is not the exact `bench.<key>:`
/// form: `Bench.expect: loud_error`, `bench expect: loud_error`, `bench_tol.rel : 1e-3`. SPICE takes such a line for a
/// plain comment, so a mistyped `expect` / `known_gap` / `tol.*` silently reverted to its default. The line must name
/// a known key after `bench` and a separator and be followed by `:`; prose such as `benchmark ...` or `bench top`
/// is not flagged. (The exact `bench.<key>:` form is `true` as well: the strict parser handles it before asking.)
pub fn looks_like_metadata(rest: &str) -> bool {
    let low = rest.trim_start().to_ascii_lowercase();
    let Some(after) = low.strip_prefix("bench") else {
        return false;
    };
    let mut chars = after.chars();
    match chars.next() {
        Some(' ' | '\t' | '.' | '_' | '-') => {}
        _ => return false,
    }
    let body = after.trim_start_matches([' ', '\t', '.', '_', '-']);
    let key: String = body
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
        .collect();
    if !KNOWN_KEYS.contains(&key.as_str()) {
        return false;
    }
    body[key.len()..].trim_start().starts_with(':')
}

/// Whether an oracle spec has one of the documented forms (SUITE_SPEC section 12): `dc_linear`, `dc_nonlinear`,
/// `dc_nonlinear(gmin=<number>)`, `tran_lti`, `ladder(n=,rs=,rsh=[,v=])`. A spec outside them is only a NOTE (the oracle
/// module may know more than this list); the case then reports oracle status `unsupported`.
pub fn oracle_spec_documented(spec: &str) -> bool {
    let n: String = spec
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let kv_ok = |body: &str, keys: &[&str]| {
        body.split(',').filter(|p| !p.is_empty()).all(|p| {
            p.split_once('=')
                .is_some_and(|(k, v)| keys.contains(&k) && parse_spice_number(v).is_some())
        })
    };
    match n.as_str() {
        "dc_linear" | "dc_nonlinear" | "tran_lti" => true,
        _ => {
            if let Some(b) = n
                .strip_prefix("dc_nonlinear(")
                .and_then(|r| r.strip_suffix(')'))
            {
                kv_ok(b, &["gmin"])
            } else if let Some(b) = n.strip_prefix("ladder(").and_then(|r| r.strip_suffix(')')) {
                kv_ok(b, &["n", "rs", "rsh", "v"])
            } else {
                false
            }
        }
    }
}

/// Parse the `* bench.*` lines of `deck`. `stem` is the file stem for static decks (id must equal it).
pub fn parse(deck: &str, stem: Option<&str>) -> ParsedMeta {
    let mut diags: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut kv: Vec<(usize, String, String)> = Vec::new();
    for (ln, line) in deck.lines().enumerate() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix('*') else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix("bench.") else {
            if ln > 0 && looks_like_metadata(rest) {
                diags.push(format!(
                    "line {}: `{}` looks like a metadata line but is not the exact `* bench.<key>: <value>` (lower case, a dot, no blank): SPICE reads it as a comment and the key would silently keep its default",
                    ln + 1,
                    line.trim()
                ));
            }
            continue;
        };
        if ln == 0 {
            diags.push(
                "line 1 is the SPICE title line; `* bench.*` metadata must come after it"
                    .to_string(),
            );
            continue;
        }
        let Some((k, v)) = rest.split_once(':') else {
            diags.push(format!(
                "line {}: metadata line without `:`: `{}`",
                ln + 1,
                line.trim()
            ));
            continue;
        };
        kv.push((ln + 1, k.trim().to_string(), v.trim().to_string()));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut m = Meta::defaults(stem.unwrap_or("unnamed"));
    let mut have_id = false;
    let mut have_group = false;
    let mut ng_solver: Option<NgSolverSel> = None;
    let mut timing: Option<bool> = None;
    for (ln, k, v) in &kv {
        let ctx = |msg: String| format!("line {ln}: bench.{k}: {msg}");
        if !KNOWN_KEYS.contains(&k.as_str()) {
            diags.push(format!("line {ln}: unknown metadata key `bench.{k}`"));
            continue;
        }
        if !seen.insert(k.clone()) {
            diags.push(ctx("duplicate key".to_string()));
            continue;
        }
        let num = |diags: &mut Vec<String>, name: &str| -> Option<f64> {
            match parse_spice_number(v) {
                Some(x) if x.is_finite() => Some(x),
                _ => {
                    diags.push(format!("line {ln}: bench.{name}: `{v}` is not a number"));
                    None
                }
            }
        };
        match k.as_str() {
            "id" => {
                have_id = true;
                m.id = v.clone();
                if !valid_id(v) {
                    diags.push(ctx(format!("`{v}` is not [a-z0-9_-]+")));
                } else if let Some(s) = stem.filter(|s| *s != v) {
                    if valid_id(s) && s.ends_with(&format!("_{v}")) {
                        // a corpus directory prefix (`rob_hs_` + id): the unique file stem is the case id
                        notes.push(format!(
                            "bench.id `{v}` lacks the file-stem prefix; the case id is the stem `{s}`"
                        ));
                        m.id = s.to_string();
                    } else {
                        diags.push(ctx(format!("id `{v}` != file stem `{s}`")));
                    }
                }
            }
            "group" => match Group::parse(v) {
                Some(g) => {
                    m.group = g;
                    have_group = true;
                }
                None => diags.push(ctx(format!(
                    "`{v}` is not capability|accuracy|robustness|performance"
                ))),
            },
            "title" => m.title = v.clone(),
            "probes" => {
                let mut ps = Vec::new();
                for p in v.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if valid_probe(p) {
                        ps.push(p.to_string());
                    } else {
                        diags.push(ctx(format!(
                            "probe `{p}` is not lower-case v(<node>) / i(<source>)"
                        )));
                    }
                }
                if ps.is_empty() && diags.is_empty() {
                    diags.push(ctx("empty probe list".to_string()));
                }
                m.probes = ps;
            }
            "tol.rel" | "tol.abs" | "tol.norm" | "floor" => {
                if let Some(x) = num(&mut diags, k) {
                    if x < 0.0 || (k == "floor" && x == 0.0) {
                        diags.push(ctx(format!("`{v}` must be non-negative (floor > 0)")));
                    } else {
                        match k.as_str() {
                            "tol.rel" => m.tol_rel = x,
                            "tol.abs" => m.tol_abs = x,
                            "tol.norm" => m.tol_norm = x,
                            _ => m.floor = x,
                        }
                    }
                }
            }
            "expect" => match v.as_str() {
                "match" => m.expect = Expect::Match,
                "loud_error" => m.expect = Expect::LoudError,
                "either" => m.expect = Expect::Either,
                _ => diags.push(ctx(format!(
                    "`{v}` is not match|loud_error|either (precise values only)"
                ))),
            },
            "known_gap" => {
                if KNOWN_GAP_SLUGS.contains(&v.as_str()) {
                    m.known_gap = Some(v.clone());
                } else {
                    diags.push(ctx(format!("`{v}` is not in the known-gap slug registry")));
                }
            }
            "gap.note" => m.gap_note = Some(v.clone()),
            "ng.options" => {
                if v.split_whitespace()
                    .all(|t| t.contains('=') || t.chars().all(|c| c.is_ascii_alphanumeric()))
                    && !v.is_empty()
                {
                    m.ng_options = Some(v.clone());
                } else {
                    diags.push(ctx(format!("`{v}` is not a list of option tokens")));
                }
            }
            "ng.solver" => match v.as_str() {
                "sparse" => ng_solver = Some(NgSolverSel::Sparse),
                "klu" => ng_solver = Some(NgSolverSel::Klu),
                "both" => ng_solver = Some(NgSolverSel::Both),
                _ => diags.push(ctx(format!("`{v}` is not sparse|klu|both"))),
            },
            "ref" => match v.as_str() {
                "tight" => m.reference = RefMode::Tight,
                "none" => m.reference = RefMode::None,
                "" => diags.push(ctx("empty value".to_string())),
                opts => {
                    if opts.split_whitespace().all(|t| t.contains('=')) {
                        m.reference = RefMode::Options(opts.to_string());
                    } else {
                        diags.push(ctx(format!(
                            "`{opts}` is not tight|none|<key=value options>"
                        )));
                    }
                }
            },
            "ref.tmax" => {
                if let Some(x) = num(&mut diags, k) {
                    if x > 0.0 {
                        m.ref_tmax = Some(x);
                    } else {
                        diags.push(ctx("tmax must be > 0".to_string()));
                    }
                }
            }
            "oracle" => {
                if v.is_empty() {
                    diags.push(ctx("empty oracle spec".to_string()));
                } else {
                    m.oracle = Some(v.clone());
                }
            }
            "series.tstep" => {
                let toks: Vec<&str> = v
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|s| !s.is_empty())
                    .collect();
                let mut ok = true;
                for t in &toks {
                    match parse_spice_number(t) {
                        Some(x) if x > 0.0 => {}
                        _ => {
                            diags.push(ctx(format!("`{t}` is not a positive time step")));
                            ok = false;
                        }
                    }
                }
                if ok && toks.len() < 2 {
                    diags.push(ctx("a series needs at least two time steps".to_string()));
                } else if ok {
                    m.series_tstep = toks.iter().map(|s| s.to_string()).collect();
                }
            }
            "size" => match v.parse::<u64>() {
                Ok(x) => m.size = Some(x),
                Err(_) => diags.push(ctx(format!("`{v}` is not an unsigned integer"))),
            },
            "tags" => {
                m.tags = v
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
            "timing" => match v.as_str() {
                "yes" => timing = Some(true),
                "no" => timing = Some(false),
                _ => diags.push(ctx(format!("`{v}` is not yes|no"))),
            },
            "est_rss_mib" => match v.parse::<u64>() {
                Ok(x) => m.est_rss_mib = Some(x),
                Err(_) => diags.push(ctx(format!("`{v}` is not an unsigned integer"))),
            },
            "note" => m.note = Some(v.clone()),
            _ => {}
        }
    }
    if !have_id {
        diags.push("missing `* bench.id:`".to_string());
    }
    if !have_group {
        diags.push("missing `* bench.group:`".to_string());
    }
    if m.gap_note.is_some() && m.known_gap.is_none() {
        diags.push("bench.gap.note without bench.known_gap".to_string());
    }
    if let Some(slug) = m.known_gap.clone() {
        // `either` is informational: a wrong answer never violates it, so the case could only read `gap_closed` and
        // the report would announce a closed gap that was never measured. The one exception is the resource
        // budget: a panic / abort / timeout / OOM is a violation even for `either` (SUITE_SPEC section 5).
        if m.expect == Expect::Either && slug != "resource-budget" {
            diags.push(format!(
                "bench.known_gap `{slug}` contradicts bench.expect: either (nothing can violate `either`, the case would read gap_closed); use match or loud_error, or drop the gap"
            ));
        }
        if m.gap_note.is_none() {
            diags.push(format!(
                "bench.known_gap `{slug}` without bench.gap.note (SUITE_SPEC 13.4.4: one line saying what RustSpice does today)"
            ));
        }
        if super::PERF_GAP_SLUGS.contains(&slug.as_str()) && m.expect != Expect::Match {
            diags.push(format!(
                "performance gap `{slug}` is judged on the measured speedup and needs bench.expect: match (found `{}`)",
                m.expect.as_str()
            ));
        }
    }
    if !m.series_tstep.is_empty() {
        // the tolerance belongs to the deck's own `.tran` step; a member coarser than that step is downgraded to
        // `info`: when EVERY member is coarser the series can never fail
        if let Some(nom) = find_tran_tokens(deck).and_then(|t| t.tstep) {
            let steps: Vec<f64> = m
                .series_tstep
                .iter()
                .filter_map(|t| parse_spice_number(t))
                .collect();
            if !steps.is_empty() && steps.iter().all(|h| *h > nom * (1.0 + 1e-9)) {
                diags.push(format!(
                    "bench.series.tstep: the deck's own .tran step {nom:e} s is finer than every series member: all members would be informational and the series could never fail; put the nominal step in the list (or fix the .tran card)"
                ));
            }
        }
    }
    if let Some(o) = &m.oracle {
        if !oracle_spec_documented(o) {
            notes.push(format!(
                "bench.oracle `{o}` is not one of the documented oracle specs (dc_linear, dc_nonlinear[(gmin=..)], tran_lti, ladder(..)): the oracle will report `unsupported` and the case has no independent truth"
            ));
        }
    }
    for t in &m.tags {
        if t.starts_with("ngspice_")
            && !ENGINE_TAGS.contains(&t.as_str())
            && !NGSPICE_NOTE_TAGS.contains(&t.as_str())
        {
            notes.push(format!(
                "tag `{t}` is not a registered ngspice_* tag (engine semantics: {}; informational: {})",
                ENGINE_TAGS.join(", "),
                NGSPICE_NOTE_TAGS.join(", ")
            ));
        }
    }
    m.ng_solver = ng_solver.unwrap_or(if m.group == Group::Performance {
        NgSolverSel::Both
    } else {
        NgSolverSel::Sparse
    });
    m.timing = timing.unwrap_or(m.group == Group::Performance || !m.series_tstep.is_empty());
    ParsedMeta {
        meta: m,
        diags,
        notes,
    }
}

/// Parse a sidecar `<deck>.sp.meta` file: the same `* bench.*` syntax, and lines without the leading `*`
/// (`bench.id: x`) are accepted too. The deck itself is fed verbatim to both simulators.
pub fn parse_sidecar(text: &str, stem: Option<&str>) -> ParsedMeta {
    let mut synthetic = String::from("sidecar metadata\n");
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        // a line that only LOOKS like metadata (`Bench.expect: x`) gets the comment marker too, so that the strict
        // parser flags it instead of the sidecar silently dropping it
        if t.starts_with("bench.") || looks_like_metadata(t) {
            synthetic.push_str("* ");
        }
        synthetic.push_str(t);
        synthetic.push('\n');
    }
    parse(&synthetic, stem)
}

/// A deck after series expansion.
#[derive(Clone, Debug)]
pub struct Expanded {
    pub id: String,
    pub deck: String,
    /// Time step (seconds) for series members.
    pub tstep: Option<f64>,
    /// `(base id, member index)` for series members.
    pub series: Option<(String, usize)>,
}

fn is_card(line: &str, card: &str) -> bool {
    let t = line.trim_start().to_ascii_lowercase();
    t.starts_with(card)
        && t[card.len()..]
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace())
}

/// Parsed `.tran` card (numeric time step and stop time).
#[derive(Clone, Debug, PartialEq)]
pub struct TranCard {
    pub line: usize,
    pub tstep: f64,
    pub tstop: f64,
    pub tstart: f64,
    pub tmax: Option<f64>,
    pub uic: bool,
    pub tstep_tok: String,
    pub tstop_tok: String,
}

/// The first `.tran` card with its tokens kept verbatim; numbers are `None` where the token is not a plain
/// number (`{1u*n}`, `abc`): such decks get their comparison grid from the ngspice rows instead.
#[derive(Clone, Debug, PartialEq)]
pub struct TranTokens {
    pub line: usize,
    pub tstep_tok: String,
    pub tstop_tok: String,
    pub tstart_tok: Option<String>,
    pub tstep: Option<f64>,
    pub tstop: Option<f64>,
    pub tstart: f64,
    pub tmax: Option<f64>,
    pub uic: bool,
}

pub fn find_tran_tokens(deck: &str) -> Option<TranTokens> {
    for (i, line) in deck.lines().enumerate() {
        if i == 0 || !is_card(line, ".tran") {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().skip(1).collect();
        if toks.len() < 2 {
            return None;
        }
        let mut uic = false;
        let mut rest: Vec<&str> = Vec::new();
        for t in &toks[2..] {
            if t.eq_ignore_ascii_case("uic") {
                uic = true;
            } else {
                rest.push(t);
            }
        }
        return Some(TranTokens {
            line: i,
            tstep_tok: toks[0].to_string(),
            tstop_tok: toks[1].to_string(),
            tstart_tok: rest.first().map(|t| t.to_string()),
            tstep: parse_spice_number(toks[0]),
            tstop: parse_spice_number(toks[1]),
            tstart: rest
                .first()
                .and_then(|t| parse_spice_number(t))
                .unwrap_or(0.0),
            tmax: rest.get(1).and_then(|t| parse_spice_number(t)),
            uic,
        });
    }
    None
}

/// The first `.tran` card when time step and stop time are plain numbers.
pub fn find_tran(deck: &str) -> Option<TranCard> {
    let t = find_tran_tokens(deck)?;
    Some(TranCard {
        line: t.line,
        tstep: t.tstep?,
        tstop: t.tstop?,
        tstart: t.tstart,
        tmax: t.tmax,
        uic: t.uic,
        tstep_tok: t.tstep_tok,
        tstop_tok: t.tstop_tok,
    })
}

fn join_lines(lines: &[String], trailing_newline: bool) -> String {
    let mut s = lines.join("\n");
    if trailing_newline {
        s.push('\n');
    }
    s
}

/// Replace the `.tran` time step with `tstep` (kept verbatim) and keep everything else.
pub fn set_tran_tstep(deck: &str, tstep: &str) -> Result<String, String> {
    let card = find_tran_tokens(deck).ok_or("deck has no parsable .tran card")?;
    let mut lines: Vec<String> = deck.lines().map(|s| s.to_string()).collect();
    let toks: Vec<&str> = lines[card.line].split_whitespace().collect();
    let mut new = vec![toks[0].to_string(), tstep.to_string()];
    new.extend(toks[2..].iter().map(|s| s.to_string()));
    lines[card.line] = new.join(" ");
    Ok(join_lines(&lines, deck.ends_with('\n')))
}

/// `.tran tstep tstop tstart tmax` (tokens of time step, stop time and start time are kept verbatim, `uic`
/// preserved).
pub fn set_tran_tmax(deck: &str, tmax: f64) -> Result<String, String> {
    let card = find_tran_tokens(deck).ok_or("deck has no parsable .tran card")?;
    let mut lines: Vec<String> = deck.lines().map(|s| s.to_string()).collect();
    let mut new = format!(
        ".tran {} {} {} {:.9e}",
        card.tstep_tok,
        card.tstop_tok,
        card.tstart_tok.as_deref().unwrap_or("0"),
        tmax
    );
    if card.uic {
        new.push_str(" uic");
    }
    lines[card.line] = new;
    Ok(join_lines(&lines, deck.ends_with('\n')))
}

fn options_line(opts: &str) -> String {
    if opts.to_ascii_lowercase().starts_with(".options")
        || opts.to_ascii_lowercase().starts_with(".option ")
    {
        opts.to_string()
    } else {
        format!(".options {opts}")
    }
}

/// Insert `.options <opts>` directly after the title line.
pub fn inject_options(deck: &str, opts: &str) -> String {
    let opts = opts.trim();
    if opts.is_empty() {
        return deck.to_string();
    }
    let line = options_line(opts);
    match deck.split_once('\n') {
        Some((title, rest)) => format!("{title}\n{line}\n{rest}"),
        None => format!("{deck}\n{line}\n"),
    }
}

/// [`inject_options`] on raw bytes, for decks that are not valid UTF-8 (every other byte stays untouched).
pub fn inject_options_bytes(deck: &[u8], opts: &str) -> Vec<u8> {
    let opts = opts.trim();
    if opts.is_empty() {
        return deck.to_vec();
    }
    let line = options_line(opts);
    let mut out = Vec::with_capacity(deck.len() + line.len() + 2);
    match deck.iter().position(|&b| b == b'\n') {
        Some(i) => {
            out.extend_from_slice(&deck[..=i]);
            out.extend_from_slice(line.as_bytes());
            out.push(b'\n');
            out.extend_from_slice(&deck[i + 1..]);
        }
        None => {
            out.extend_from_slice(deck);
            out.push(b'\n');
            out.extend_from_slice(line.as_bytes());
            out.push(b'\n');
        }
    }
    out
}

/// Expand `bench.series.tstep` into one deck per time step; non-series decks yield one unchanged entry.
pub fn expand_series(deck: &str, meta: &Meta) -> Result<Vec<Expanded>, String> {
    if meta.series_tstep.is_empty() {
        return Ok(vec![Expanded {
            id: meta.id.clone(),
            deck: deck.to_string(),
            tstep: None,
            series: None,
        }]);
    }
    let mut out = Vec::new();
    for (k, ts) in meta.series_tstep.iter().enumerate() {
        let id = format!("{}__s{}", meta.id, k + 1);
        let tstep = parse_spice_number(ts).ok_or_else(|| format!("bad series tstep `{ts}`"))?;
        let swapped = set_tran_tstep(deck, ts)?;
        let mut lines: Vec<String> = Vec::new();
        for (i, l) in swapped.lines().enumerate() {
            let t = l.trim_start();
            let is_meta = i > 0 && t.starts_with('*') && t[1..].trim_start().starts_with("bench.");
            if is_meta {
                let key = t[1..].trim_start()["bench.".len()..]
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if key == "series.tstep" {
                    continue;
                }
                if key == "id" {
                    lines.push(format!("* bench.id: {id}"));
                    continue;
                }
            }
            lines.push(l.to_string());
        }
        out.push(Expanded {
            id,
            deck: join_lines(&lines, deck.ends_with('\n')),
            tstep: Some(tstep),
            series: Some((meta.id.clone(), k + 1)),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECK: &str = "title line\n* bench.id: rc_tran\n* bench.group: accuracy\n* bench.title: RC\n* bench.probes: v(out), i(v1)\n* bench.tol.norm: 5e-3\n* bench.expect: match\nV1 in 0 PULSE(0 1 0 1n 1n 1u 2u)\nR1 in out 1k\nC1 out 0 1n\n.tran 10n 1u\n.end\n";

    #[test]
    fn numbers_with_suffixes() {
        assert_eq!(parse_spice_number("10p"), Some(10e-12));
        assert_eq!(parse_spice_number("1meg"), Some(1e6));
        assert_eq!(parse_spice_number("2.5u"), Some(2.5e-6));
        assert_eq!(parse_spice_number("1e-9"), Some(1e-9));
        assert_eq!(parse_spice_number("5nsec"), Some(5e-9));
        assert_eq!(parse_spice_number("3V"), Some(3.0));
        assert_eq!(parse_spice_number("1k"), Some(1000.0));
        assert_eq!(parse_spice_number("abc"), None);
        assert_eq!(parse_spice_number("1e999"), None);
        assert_eq!(parse_spice_number(""), None);
    }

    #[test]
    fn parses_valid_metadata() {
        let p = parse(DECK, Some("rc_tran"));
        assert!(p.diags.is_empty(), "{:?}", p.diags);
        assert_eq!(p.meta.group, Group::Accuracy);
        assert_eq!(p.meta.probes, vec!["v(out)", "i(v1)"]);
        assert_eq!(p.meta.expect, Expect::Match);
        assert_eq!(p.meta.tol_norm, 5e-3);
        assert_eq!(p.meta.ng_solver, NgSolverSel::Sparse);
        assert!(!p.meta.timing);
    }

    #[test]
    fn defaults_follow_group() {
        let d = "t\n* bench.id: x\n* bench.group: performance\n.op\n.end\n";
        let p = parse(d, Some("x"));
        assert!(p.diags.is_empty());
        assert_eq!(p.meta.ng_solver, NgSolverSel::Both);
        assert!(p.meta.timing);
    }

    #[test]
    fn strict_diagnostics() {
        let bad = |extra: &str| {
            let d = format!("t\n* bench.id: x\n* bench.group: capability\n{extra}\n.op\n.end\n");
            parse(&d, Some("x")).diags
        };
        assert!(bad("* bench.expect: maybe")[0].contains("match|loud_error|either"));
        assert!(bad("* bench.frobnicate: 1")[0].contains("unknown metadata key"));
        assert!(bad("* bench.tol.rel: abc")[0].contains("not a number"));
        assert!(bad("* bench.known_gap: made-up-slug")[0].contains("registry"));
        assert!(bad("* bench.known_gap: clamp-rail\n* bench.gap.note: n").is_empty());
        assert!(bad("* bench.ng.solver: umfpack")[0].contains("sparse|klu|both"));
        assert!(bad("* bench.probes: V(Out)")[0].contains("lower-case"));
        assert!(bad("* bench.tol.rel: 1e-6\n* bench.tol.rel: 1e-7")[0].contains("duplicate"));
        assert!(bad("* bench.series.tstep: 10p")[0].contains("at least two"));
        assert!(bad("* bench.gap.note: why")[0].contains("without bench.known_gap"));
        // id != stem, missing id/group
        let d = "t\n* bench.id: a\n* bench.group: capability\n.op\n";
        assert!(parse(d, Some("b")).diags[0].contains("!= file stem"));
        let d = "t\n.op\n";
        let diags = parse(d, Some("b")).diags;
        assert!(diags.iter().any(|x| x.contains("missing `* bench.id:`")));
        assert!(diags.iter().any(|x| x.contains("missing `* bench.group:`")));
        // metadata on the title line is rejected
        let d = "* bench.id: x\n* bench.group: capability\n.op\n";
        assert!(parse(d, Some("x")).diags[0].contains("title line"));
    }

    /// ER-driver-17: a mistyped key is a comment to SPICE and silently kept its default (expect=match).
    #[test]
    fn lookalike_metadata_lines_are_diagnosed_not_ignored() {
        let bad = |line: &str| {
            let d = format!("t\n* bench.id: x\n* bench.group: capability\n{line}\n.op\n.end\n");
            parse(&d, Some("x")).diags
        };
        for l in [
            "* Bench.expect: loud_error",
            "* bench expect: loud_error",
            "* BENCH.EXPECT: loud_error",
            "* bench_tol.rel : 1e-3",
            "*bench-known_gap: clamp-rail",
            "*   Bench.group: robustness",
        ] {
            let d = bad(l);
            assert_eq!(d.len(), 1, "{l}: {d:?}");
            assert!(d[0].contains("looks like a metadata line"), "{l}: {d:?}");
        }
        // prose and unrelated comments are not metadata
        for l in [
            "* benchmark circuit from the paper",
            "* bench top supply: 5 V",
            "* bench marking: nothing",
            "* the bench.expect key is described in the README",
            "* benchmarks: none",
            "* bench: 1",
            "* bench frobnicate: 1",
        ] {
            assert!(bad(l).is_empty(), "{l}: {:?}", bad(l));
        }
        // a sidecar line with the same typo is diagnosed instead of silently dropped
        let p = parse_sidecar(
            "bench.group: robustness\nBench.expect: loud_error\nbench.id: e\n",
            Some("e"),
        );
        assert!(
            p.diags
                .iter()
                .any(|d| d.contains("looks like a metadata line")),
            "{:?}",
            p.diags
        );
        assert!(looks_like_metadata("bench.expect: x") && !looks_like_metadata("benchmark: x"));
    }

    /// ER-driver-06: contradictory known-gap metadata is invalid, not silently `gap_closed`.
    #[test]
    fn contradictory_known_gap_metadata_is_invalid() {
        let diags = |extra: &str| {
            let d = format!("t\n* bench.id: x\n* bench.group: capability\n{extra}\n.op\n.end\n");
            parse(&d, Some("x")).diags
        };
        // known_gap with expect: either can only ever read gap_closed
        let d = diags("* bench.expect: either\n* bench.known_gap: clamp-rail\n* bench.gap.note: n");
        assert!(
            d.iter()
                .any(|x| x.contains("contradicts bench.expect: either")),
            "{d:?}"
        );
        // ... except the resource budget, where a panic/timeout violates `either` too
        assert!(diags(
            "* bench.expect: either\n* bench.known_gap: resource-budget\n* bench.gap.note: n"
        )
        .is_empty());
        // a gap without the one-line note (13.4.4)
        let d = diags("* bench.known_gap: clamp-rail");
        assert!(
            d.iter().any(|x| x.contains("without bench.gap.note")),
            "{d:?}"
        );
        // a performance slug is judged on speed and needs expect: match
        assert!(diags("* bench.known_gap: faer-nonlinear-slow\n* bench.gap.note: n").is_empty());
        let d = diags(
            "* bench.expect: either\n* bench.known_gap: faer-nonlinear-slow\n* bench.gap.note: n",
        );
        assert!(
            d.iter().any(|x| x.contains("needs bench.expect: match")),
            "{d:?}"
        );
        let d = diags("* bench.expect: loud_error\n* bench.known_gap: faer-nonlinear-slow\n* bench.gap.note: n");
        assert!(
            d.iter().any(|x| x.contains("needs bench.expect: match")),
            "{d:?}"
        );
        // loud_error with an ordinary gap is fine
        assert!(diags(
            "* bench.expect: loud_error\n* bench.known_gap: clamp-rail\n* bench.gap.note: n"
        )
        .is_empty());
    }

    /// ER-driver-07: a nominal `.tran` step finer than every member downgraded the whole series to `info`.
    #[test]
    fn a_series_whose_nominal_step_is_finer_than_every_member_is_invalid() {
        let deck = |tran: &str, list: &str| {
            format!("t\n* bench.id: s\n* bench.group: accuracy\n* bench.series.tstep: {list}\nV1 a 0 1\nR1 a 0 1\n{tran}\n.end\n")
        };
        let d = parse(&deck(".tran 1n 1u", "50n 20n 10n"), Some("s")).diags;
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("finer than every series member"), "{d:?}");
        // the nominal step is in the list, or some member is finer: fine
        assert!(parse(&deck(".tran 10n 1u", "50n 20n 10n"), Some("s"))
            .diags
            .is_empty());
        assert!(parse(&deck(".tran 20n 1u", "50n 20n 10n"), Some("s"))
            .diags
            .is_empty());
        assert!(parse(&deck(".tran 20n 1u", "50n 10n"), Some("s"))
            .diags
            .is_empty());
        // a parameterised card has no numeric nominal step: nothing to compare
        assert!(parse(&deck(".tran {n} 1u", "50n 20n"), Some("s"))
            .diags
            .is_empty());
    }

    #[test]
    fn an_undocumented_oracle_spec_is_a_visible_note() {
        for ok in [
            "dc_linear",
            "dc_nonlinear",
            "DC_Nonlinear(gmin=1e-12)",
            "tran_lti",
            "ladder(n=10, rs=1k, rsh=2k)",
            "ladder(n=10,rs=1k,rsh=2k,v=2)",
        ] {
            assert!(oracle_spec_documented(ok), "{ok}");
        }
        for bad in [
            "dc_linaer",
            "dc_nonlinear(gmin)",
            "dc_nonlinear(gmim=1e-3)",
            "ladder(n=x)",
            "tran",
            "",
        ] {
            assert!(!oracle_spec_documented(bad), "{bad}");
        }
        let d = "t\n* bench.id: x\n* bench.group: accuracy\n* bench.oracle: dc_linaer\n.op\n";
        let p = parse(d, Some("x"));
        assert!(
            p.diags.is_empty(),
            "a note, not a diagnostic: {:?}",
            p.diags
        );
        assert!(
            p.notes
                .iter()
                .any(|n| n.contains("dc_linaer") && n.contains("unsupported")),
            "{:?}",
            p.notes
        );
        let d = "t\n* bench.id: x\n* bench.group: accuracy\n* bench.oracle: dc_linear\n.op\n";
        assert!(parse(d, Some("x")).notes.is_empty());
    }

    #[test]
    fn tran_card_rewriting() {
        let c = find_tran(DECK).unwrap();
        assert_eq!((c.tstep, c.tstop, c.tmax), (10e-9, 1e-6, None));
        let d = set_tran_tmax(DECK, 5e-10).unwrap();
        let c2 = find_tran(&d).unwrap();
        assert_eq!(
            (c2.tstep, c2.tstop, c2.tstart, c2.tmax),
            (10e-9, 1e-6, 0.0, Some(5e-10))
        );
        assert!(d.ends_with(".end\n"));
        let d = set_tran_tstep("t\n.TRAN 1n 1u 0 2n uic\n.end\n", "5n").unwrap();
        let c3 = find_tran(&d).unwrap();
        assert_eq!((c3.tstep, c3.tmax, c3.uic), (5e-9, Some(2e-9), true));
        assert!(set_tran_tmax("t\n.op\n", 1e-9).is_err());
    }

    #[test]
    fn option_injection_goes_after_title() {
        let d = inject_options("title\nR1 a 0 1\n.op\n", "klu");
        assert_eq!(d, "title\n.options klu\nR1 a 0 1\n.op\n");
        assert_eq!(inject_options("t\n.op\n", ""), "t\n.op\n");
        assert_eq!(
            inject_options("t\n.op\n", ".options reltol=1e-9"),
            "t\n.options reltol=1e-9\n.op\n"
        );
    }

    #[test]
    fn series_expansion() {
        let d = DECK.replace(
            "* bench.tol.norm: 5e-3\n",
            "* bench.series.tstep: 10n 5n 2n\n",
        );
        let p = parse(&d, Some("rc_tran"));
        assert!(p.diags.is_empty(), "{:?}", p.diags);
        let ex = expand_series(&d, &p.meta).unwrap();
        assert_eq!(ex.len(), 3);
        assert_eq!(ex[1].id, "rc_tran__s2");
        assert_eq!(ex[1].tstep, Some(5e-9));
        assert!(ex[1].deck.contains("* bench.id: rc_tran__s2"));
        assert!(!ex[1].deck.contains("series.tstep"));
        assert_eq!(find_tran(&ex[2].deck).unwrap().tstep, 2e-9);
        assert!(p.meta.timing);
    }

    #[test]
    fn slug_registry_has_the_phase2_additions() {
        assert_eq!(KNOWN_GAP_SLUGS.len(), 40);
        let mut uniq: Vec<&str> = KNOWN_GAP_SLUGS.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 40, "duplicate slug in the registry");
        for slug in [
            "unsupported-syntax-loud",
            "unsupported-device-loud",
            "lenient-malformed-input",
            "silent-misparse",
            "dc-value-ignored-with-waveform",
            "dc-static-homotopy",
            "exp-argument-clamp",
            "ill-posed-dc-silent",
            "title-line-parsed-as-card",
            "nonfinite-result-accepted",
            "step-retry-grid-desync",
            "newton-iteration-cap",
            "param-floor-clamp",
        ] {
            let d = format!(
                "t\n* bench.id: x\n* bench.group: capability\n* bench.known_gap: {slug}\n* bench.gap.note: n\n.op\n"
            );
            let p = parse(&d, Some("x"));
            assert!(p.diags.is_empty(), "{slug}: {:?}", p.diags);
            assert_eq!(p.meta.known_gap.as_deref(), Some(slug));
        }
    }

    #[test]
    fn id_may_lack_the_corpus_directory_prefix() {
        // `rob_hs_` + id: accepted, the unique stem becomes the case id and a note says so
        let d = "t\n* bench.id: syn_x\n* bench.group: robustness\n.op\n";
        let p = parse(d, Some("rob_hs_syn_x"));
        assert!(p.diags.is_empty(), "{:?}", p.diags);
        assert_eq!(p.meta.id, "rob_hs_syn_x");
        assert!(p.notes[0].contains("lacks the file-stem prefix"));
        // an unrelated id is still an error, so is a suffix that is not on a word boundary
        assert!(parse(d, Some("rob_hs_other")).diags[0].contains("!= file stem"));
        assert!(parse(d, Some("robsyn_x")).diags[0].contains("!= file stem"));
        // equal ids carry no note
        assert!(parse(d, Some("syn_x")).notes.is_empty());
    }

    #[test]
    fn sidecar_accepts_both_line_styles() {
        let side = "* bench.id: empty_one\nbench.group: robustness\n\n  bench.expect: loud_error  \n# not metadata\n* bench.tags: file, ngspice_crash\n";
        let p = parse_sidecar(side, Some("empty_one"));
        assert!(p.diags.is_empty(), "{:?}", p.diags);
        assert_eq!(p.meta.group, Group::Robustness);
        assert_eq!(p.meta.expect, Expect::LoudError);
        assert!(p.meta.has_tag("ngspice_crash"));
        // a sidecar cannot be silent about its id and group
        let p = parse_sidecar("bench.expect: loud_error\n", Some("e"));
        assert!(p.diags.iter().any(|d| d.contains("missing `* bench.id:`")));
    }

    #[test]
    fn tran_card_tokens_survive_parameterised_cards() {
        let d = "t\n.param n=5\n.tran 10n {1u*n} 2u 5n UIC\n.end\n";
        assert!(find_tran(d).is_none(), "not numeric: no plain card");
        let t = find_tran_tokens(d).unwrap();
        assert_eq!(t.tstep, Some(10e-9));
        assert_eq!(t.tstop, None);
        assert_eq!((t.tstart, t.tmax, t.uic), (2e-6, Some(5e-9), true));
        // the reference deck keeps the expression verbatim and the start time token
        let out = set_tran_tmax(d, 5e-10).unwrap();
        assert!(
            out.contains(".tran 10n {1u*n} 2u 5.000000000e-10 uic"),
            "{out}"
        );
        let out = set_tran_tstep(d, "5n").unwrap();
        assert!(out.contains(".tran 5n {1u*n} 2u 5n UIC"), "{out}");
    }

    #[test]
    fn options_injection_is_byte_exact_for_non_utf8_decks() {
        let deck: &[u8] = b"t\xe9\xff title\nR1 a 0 1\n* c\x00x\n.op\n";
        assert!(String::from_utf8(deck.to_vec()).is_err());
        let out = inject_options_bytes(deck, "klu");
        assert_eq!(
            out,
            b"t\xe9\xff title\n.options klu\nR1 a 0 1\n* c\x00x\n.op\n".to_vec()
        );
        assert_eq!(inject_options_bytes(deck, " "), deck.to_vec());
        assert_eq!(
            inject_options_bytes(b"x", "klu"),
            b"x\n.options klu\n".to_vec()
        );
    }

    #[test]
    fn probes_accept_schematic_node_names_and_decks_may_select_klu() {
        for ok in [
            "v(n+)",
            "v(bus[1])",
            "v(a/b)",
            "v(x:y)",
            "i(v1)",
            "v(1)",
            "v(n-)",
        ] {
            assert!(valid_probe(ok), "{ok}");
        }
        for bad in ["V(a)", "v()", "v(a,b)", "v(a b)", "x(a)", "v(a", "v(A)"] {
            assert!(!valid_probe(bad), "{bad}");
        }
        assert!(deck_selects_klu(
            "t\nR1 a 0 1\n.options reltol=1e-6 KLU\n.op\n"
        ));
        assert!(deck_selects_klu("t\n.option klu\n"));
        assert!(!deck_selects_klu(
            "t\n.options noklu\n* .options klu\nR1 a 0 klu\n"
        ));
    }

    #[test]
    fn unregistered_ngspice_tags_are_flagged() {
        let d = "t\n* bench.id: x\n* bench.group: capability\n* bench.tags: ngspice_error_ok, ngspice_warns, ngspice_typo, family:ladder\n.op\n";
        let p = parse(d, Some("x"));
        assert!(p.diags.is_empty());
        assert_eq!(p.notes.len(), 1, "{:?}", p.notes);
        assert!(p.notes[0].contains("ngspice_typo"));
        assert!(p.meta.ngspice_error_ok() && !p.meta.ngspice_unbounded());
    }
}
