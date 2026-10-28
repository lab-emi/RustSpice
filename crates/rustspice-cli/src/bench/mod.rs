//! Differential benchmark suite engine (`rustspice bench-suite`) — see `benchmarks/README.md`.
//!
//! Module ownership: H1 engine (this file, meta, rawfile, ngspice, rustsim, compare, stats, report),
//! H2 generators (families, fuzz, oracle), H3 legacy (legacy).
#![allow(dead_code)]

pub mod batch_gpu;
pub mod compare;
pub mod families;
pub mod fuzz;
pub mod legacy;
pub mod meta;
pub mod ngspice;
pub mod oracle;
pub mod rawfile;
pub mod report;
pub mod rustsim;
pub mod stats;
pub mod teeth;
pub mod types;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Args;

use compare::{
    classify, compare_with, detect_kinds, CompareInput, CompareResult, NgSide, RsSide, Status,
};
use meta::{Expect, Group, Meta, NgSolver, RefMode, TIGHT_OPTIONS, TRAN_REF_OPTIONS};
use ngspice::NgClass;
use rawfile::Plot;
use report::*;
use rustsim::{Limits, RsClass, TmpDir};
use stats::{loglog_fit, summarize, time_at_equal_error, NOISY_THRESHOLD};
use teeth::{MutationResult, TeethResult, TeethSummary};
use types::Scale;

/// Known-gap slugs that describe a performance shortfall rather than a wrong/absent answer: they are judged on
/// the measured speedup instead of on `bench.expect`.
pub const PERF_GAP_SLUGS: &[&str] = &["faer-nonlinear-slow"];

/// Arguments of `rustspice bench-suite` (SUITE_SPEC section 7).
#[derive(Args, Debug, Clone)]
pub struct SuiteArgs {
    /// Directory of `*.sp` benchmark decks (searched recursively)
    #[arg(long, default_value = "benchmarks/cases")]
    pub cases: PathBuf,
    /// Only these groups (capability, accuracy, robustness, performance); repeatable
    #[arg(long)]
    pub group: Vec<String>,
    /// Only cases whose id contains this substring
    #[arg(long)]
    pub filter: Option<String>,
    /// Only these families (generated family id or `family:<id>` tag); repeatable
    #[arg(long)]
    pub family: Vec<String>,
    /// Generated-deck scale: small (CI smoke), medium (default), large (opt-in, memory estimate printed)
    #[arg(long, default_value = "medium")]
    pub scale: String,
    /// Timed repetitions per case (interleaved A/B; the correctness run is the warm-up)
    #[arg(long, default_value_t = 7)]
    pub reps: usize,
    /// Wall-clock limit per child process in seconds
    #[arg(long, default_value_t = 60)]
    pub case_timeout: u64,
    /// Resident-memory cap per child process in MiB; cases estimated above it are skipped
    #[arg(long, default_value_t = 4096)]
    pub max_rss_mib: u64,
    /// Seed for the generators and fuzzers
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Fuzz decks per family (default 25/100/400 by scale); mutants = 2x this
    #[arg(long)]
    pub fuzz_count: Option<usize>,
    /// Write the JSON report here
    #[arg(long)]
    pub json: Option<PathBuf>,
    /// Write the Markdown report here
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Keep decks, raw files, logs of non-passing cases here (default `bench-artifacts`, created lazily)
    #[arg(long)]
    pub artifacts: Option<PathBuf>,
    /// ngspice executable
    #[arg(long, default_value = "ngspice")]
    pub ngspice: String,
    /// Pin child processes to this CPU
    #[arg(long)]
    pub pin_cpu: Option<usize>,
    /// Skip all timing repetitions
    #[arg(long)]
    pub no_timing: bool,
    /// Skip generated fuzz decks and mutants
    #[arg(long)]
    pub no_fuzz: bool,
    /// Mutation-sensitivity check: for every passing numeric case re-run RustSpice alone on decks with the first, a
    /// middle and the last value-carrying element scaled by 1.05 and compare with the ORIGINAL ngspice data; a pass
    /// that survives every mutation is reported as `teeth: blunt` (never changes `status`)
    #[arg(long)]
    pub teeth: bool,
    /// List the selected cases and exit
    #[arg(long)]
    pub list: bool,
    /// Exit 0 even if ngspice is missing or not usable (every case is then `unverified`)
    #[arg(long)]
    pub allow_unverified: bool,
    /// Exit 2 when any case is `unverified` (invalid metadata, an unjudged performance gap, a harness I/O failure, a
    /// reference that fell back, ...) even if nothing failed. Without it the cases are listed in the report and the
    /// verdict line, but the exit code stays 0.
    #[arg(long)]
    pub strict_unverified: bool,
    /// Run only the generated families (and fuzz decks): the static corpus under `--cases` is neither required nor
    /// read. Without this flag a `--cases` directory without a single deck is an error (a mistyped path used to give
    /// a green run of the generated decks alone).
    #[arg(long)]
    pub families_only: bool,
}

// ---------------------------------------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Static,
    Family,
    Fuzz,
    Mutant,
}

impl Origin {
    fn as_str(self) -> &'static str {
        match self {
            Origin::Static => "static",
            Origin::Family => "family",
            Origin::Fuzz => "fuzz",
            Origin::Mutant => "mutant",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub meta: Meta,
    pub deck: String,
    pub family: String,
    pub origin: Origin,
    pub size: Option<u64>,
    pub est_rss_mib: Option<u64>,
    pub diags: Vec<String>,
    pub series: Option<(String, usize)>,
    pub tstep: Option<f64>,
    /// Series members: the time step of the base deck's own `.tran` card (the tolerance is defined for it).
    pub nominal_tstep: Option<f64>,
    /// The file's bytes when it is not valid UTF-8 (`deck` is then lossy and only used for the metadata); both
    /// simulators are always fed the original bytes.
    pub raw: Option<Vec<u8>>,
    /// Directory of the deck file: both simulators run there so relative `.include`/`.lib` targets resolve.
    pub src_dir: Option<PathBuf>,
    /// Non-blocking metadata remarks (sidecar used, id prefix, ...).
    pub notes: Vec<String>,
}

/// Where a static deck came from (everything the text alone does not tell).
#[derive(Clone, Debug, Default)]
struct DeckSource {
    /// Text of `<deck>.sp.meta` when that sidecar exists: it replaces the in-deck `* bench.*` lines.
    sidecar: Option<String>,
    raw: Option<Vec<u8>>,
    dir: Option<PathBuf>,
}

/// Deepest directory level the corpus walk descends to (a directory tree deeper than this is an error, not a
/// silent truncation).
const MAX_WALK_DEPTH: usize = 16;

/// Result of walking the corpus directory.
#[derive(Debug, Default)]
struct Walked {
    files: Vec<PathBuf>,
    /// Directories or entries that could not be read: a deck below them would be silently missing from the run.
    errors: Vec<String>,
    /// Symbolic links to directories, which are NOT followed (a link back into an ancestor loops, two self-links
    /// made the walk exponential).
    skipped_links: Vec<PathBuf>,
}

fn is_deck_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("sp") | Some("cir") | Some("spice")
    )
}

fn walk(dir: &Path, depth: usize, w: &mut Walked) {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            w.errors.push(format!("{}: {e}", dir.display()));
            return;
        }
    };
    let mut entries: Vec<(PathBuf, std::fs::FileType)> = Vec::new();
    for e in rd {
        match e.and_then(|e| e.file_type().map(|t| (e.path(), t))) {
            Ok(x) => entries.push(x),
            Err(err) => w.errors.push(format!("{}: {err}", dir.display())),
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (p, ft) in entries {
        if ft.is_dir() {
            if depth >= MAX_WALK_DEPTH {
                w.errors.push(format!(
                    "{}: nested deeper than {MAX_WALK_DEPTH} directory levels",
                    p.display()
                ));
            } else {
                walk(&p, depth + 1, w);
            }
        } else if ft.is_symlink() {
            match std::fs::metadata(&p) {
                Ok(m) if m.is_dir() => w.skipped_links.push(p),
                Ok(_) => {
                    if is_deck_file(&p) {
                        w.files.push(p);
                    }
                }
                Err(e) => {
                    if is_deck_file(&p) {
                        w.errors
                            .push(format!("{}: dangling symbolic link: {e}", p.display()));
                    }
                }
            }
        } else if ft.is_file() && is_deck_file(&p) {
            w.files.push(p);
        }
    }
}

/// Turn one deck (static or generated) into cases (several for a `bench.series.tstep` deck).
fn make_cases(
    deck: &str,
    stem: Option<&str>,
    family_hint: Option<&str>,
    origin: Origin,
    size: Option<u64>,
    est: Option<u64>,
    out: &mut Vec<Case>,
) {
    make_cases_from(
        deck,
        &DeckSource::default(),
        stem,
        family_hint,
        origin,
        size,
        est,
        out,
    );
}

#[allow(clippy::too_many_arguments)]
fn make_cases_from(
    deck: &str,
    src: &DeckSource,
    stem: Option<&str>,
    family_hint: Option<&str>,
    origin: Origin,
    size: Option<u64>,
    est: Option<u64>,
    out: &mut Vec<Case>,
) {
    let parsed = match &src.sidecar {
        Some(side) => meta::parse_sidecar(side, stem),
        None => meta::parse(deck, stem),
    };
    let mut notes = parsed.notes;
    if src.sidecar.is_some() {
        notes.push("metadata read from the .sp.meta sidecar; the deck is fed verbatim".to_string());
    }
    let mut m = parsed.meta;
    if m.size.is_none() {
        m.size = size;
    }
    let family = family_hint
        .map(|s| s.to_string())
        .or_else(|| m.family_tag().map(|s| s.to_string()))
        .unwrap_or_else(|| "static".to_string());
    let est_rss = est.or(m.est_rss_mib);
    let mut diags = parsed.diags;
    if diags.is_empty()
        && !m.series_tstep.is_empty()
        && (src.sidecar.is_some() || src.raw.is_some())
    {
        diags.push("bench.series.tstep needs a plain UTF-8 deck without a sidecar".to_string());
    }
    let nominal = if m.series_tstep.is_empty() {
        None
    } else {
        meta::find_tran_tokens(deck).and_then(|t| t.tstep)
    };
    if diags.is_empty() {
        match meta::expand_series(deck, &m) {
            Ok(ex) => {
                for e in ex {
                    let mut mm = m.clone();
                    mm.id = e.id.clone();
                    out.push(Case {
                        id: e.id,
                        meta: mm,
                        deck: e.deck,
                        family: family.clone(),
                        origin,
                        size: m.size,
                        est_rss_mib: est_rss,
                        diags: Vec::new(),
                        series: e.series,
                        tstep: e.tstep,
                        nominal_tstep: nominal,
                        raw: src.raw.clone(),
                        src_dir: src.dir.clone(),
                        notes: notes.clone(),
                    });
                }
                return;
            }
            Err(e) => diags.push(format!("series expansion: {e}")),
        }
    }
    let id = if m.id.is_empty() || m.id == "unnamed" {
        stem.unwrap_or("unnamed").to_string()
    } else {
        m.id.clone()
    };
    out.push(Case {
        id,
        meta: m.clone(),
        deck: deck.to_string(),
        family,
        origin,
        size: m.size,
        est_rss_mib: est_rss,
        diags,
        series: None,
        tstep: None,
        nominal_tstep: None,
        raw: src.raw.clone(),
        src_dir: src.dir.clone(),
        notes,
    });
}

fn load_cases(args: &SuiteArgs, scale: Scale, fuzz_count: usize) -> Result<Vec<Case>, String> {
    let mut cases: Vec<Case> = Vec::new();
    let mut corpus: Vec<(String, String)> = Vec::new();
    let mut walked = Walked::default();
    if !args.families_only {
        // a mistyped or relative-from-elsewhere `--cases` must not turn into a green run of the generated decks alone
        if !args.cases.is_dir() {
            return Err(format!(
                "--cases `{}` is not a directory (current directory `{}`)",
                args.cases.display(),
                std::env::current_dir().map_or("?".to_string(), |d| d.display().to_string())
            ));
        }
        walk(&args.cases, 0, &mut walked);
        if !walked.errors.is_empty() {
            return Err(format!(
                "cannot read the cases directory tree: {}",
                walked.errors.join("; ")
            ));
        }
        for l in &walked.skipped_links {
            eprintln!(
                "warning: `{}` is a symbolic link to a directory and is not followed",
                l.display()
            );
        }
        if walked.files.is_empty() {
            return Err(format!(
                "no benchmark decks (*.sp, *.cir, *.spice) under `{}` (pass --families-only to run the generated decks alone)",
                args.cases.display()
            ));
        }
    }
    for f in std::mem::take(&mut walked.files) {
        let bytes = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let (text, raw) = match String::from_utf8(bytes) {
            Ok(t) => (t, None),
            Err(e) => {
                let b = e.into_bytes();
                (String::from_utf8_lossy(&b).to_string(), Some(b))
            }
        };
        let stem = f
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let mut side_path = f.clone().into_os_string();
        side_path.push(".meta");
        let sidecar = match std::fs::read(&side_path) {
            Ok(b) => Some(String::from_utf8_lossy(&b).to_string()),
            Err(_) => None,
        };
        let dir = f
            .parent()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));
        let src = DeckSource { sidecar, raw, dir };
        let before = cases.len();
        make_cases_from(
            &text,
            &src,
            Some(&stem),
            None,
            Origin::Static,
            None,
            None,
            &mut cases,
        );
        if cases[before..].iter().all(|c| c.diags.is_empty())
            && text.len() <= 1 << 20
            && src.raw.is_none()
            && src.sidecar.is_none()
        {
            corpus.push((stem, text));
        }
    }
    for g in families::generate(scale, args.seed) {
        make_cases(
            &g.deck,
            Some(&g.id),
            Some(&g.family),
            Origin::Family,
            Some(g.size),
            Some(g.est_rss_mib),
            &mut cases,
        );
    }
    if !args.no_fuzz {
        for g in fuzz::fuzz_decks(fuzz_count, args.seed) {
            make_cases(
                &g.deck,
                Some(&g.id),
                Some(&g.family),
                Origin::Fuzz,
                Some(g.size),
                Some(g.est_rss_mib),
                &mut cases,
            );
        }
        for g in fuzz::mutants(&corpus, fuzz_count * 2, args.seed) {
            make_cases(
                &g.deck,
                Some(&g.id),
                Some(&g.family),
                Origin::Mutant,
                Some(g.size),
                Some(g.est_rss_mib),
                &mut cases,
            );
        }
    }
    // duplicate ids become diagnostics, never silently dropped
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen: HashSet<String> = HashSet::new();
    for c in cases.iter_mut() {
        if !seen.insert(c.id.clone()) {
            c.diags.push(format!("duplicate case id `{}`", c.id));
        }
    }
    // filters
    let groups: Vec<Group> = args.group.iter().filter_map(|g| Group::parse(g)).collect();
    cases.retain(|c| {
        (groups.is_empty() || groups.contains(&c.meta.group))
            && args
                .filter
                .as_ref()
                .is_none_or(|f| c.id.contains(f.as_str()))
            && (args.family.is_empty()
                || args.family.iter().any(|f| {
                    *f == c.family
                        || c.meta
                            .tags
                            .iter()
                            .any(|t| t == &format!("family:{f}") || t == f)
                }))
    });
    Ok(cases)
}

// ---------------------------------------------------------------------------------------------------------
// ngspice deck construction
// ---------------------------------------------------------------------------------------------------------

/// Deck text for an ngspice run: optional `tmax` on the `.tran` card, extra options, and `.options klu`.
pub fn ng_deck_text(
    deck: &str,
    solver: NgSolver,
    extra: Option<&str>,
    tmax: Option<f64>,
) -> Result<String, String> {
    let mut d = deck.to_string();
    if let Some(tm) = tmax {
        d = meta::set_tran_tmax(&d, tm)?;
    }
    if let Some(o) = extra {
        d = meta::inject_options(&d, o);
    }
    if solver == NgSolver::Klu {
        d = meta::inject_options(&d, "klu");
    }
    Ok(d)
}

/// [`ng_deck_text`] on a case: the original bytes (non-UTF-8 decks stay byte-exact; only an option line is
/// inserted after the title line, a `.tran` rewrite is refused).
fn ng_deck_bytes(
    case: &Case,
    solver: NgSolver,
    extra: Option<&str>,
    tmax: Option<f64>,
) -> Result<Vec<u8>, String> {
    let Some(raw) = &case.raw else {
        return ng_deck_text(&case.deck, solver, extra, tmax).map(String::into_bytes);
    };
    if tmax.is_some() {
        return Err("cannot rewrite the .tran card of a non-UTF-8 deck".to_string());
    }
    let mut d = raw.clone();
    if let Some(o) = extra {
        d = meta::inject_options_bytes(&d, o);
    }
    if solver == NgSolver::Klu {
        d = meta::inject_options_bytes(&d, "klu");
    }
    Ok(d)
}

struct RefSpec {
    options: Option<String>,
    tmax: Option<f64>,
    label: &'static str,
}

fn ref_spec(case: &Case, has_tran: bool) -> RefSpec {
    let m = &case.meta;
    let base_tstep = m
        .series_tstep
        .iter()
        .filter_map(|t| meta::parse_spice_number(t))
        .fold(f64::INFINITY, f64::min)
        .min(
            meta::find_tran_tokens(&case.deck)
                .and_then(|c| c.tstep)
                .unwrap_or(f64::INFINITY),
        );
    let auto_tmax = if base_tstep.is_finite() && base_tstep > 0.0 {
        Some(base_tstep / 20.0)
    } else {
        None
    };
    match &m.reference {
        RefMode::None => RefSpec {
            options: None,
            tmax: None,
            label: "as_is",
        },
        RefMode::Default => {
            if has_tran {
                RefSpec {
                    options: Some(TRAN_REF_OPTIONS.to_string()),
                    tmax: m.ref_tmax.or(auto_tmax),
                    label: "converged_tran",
                }
            } else {
                RefSpec {
                    options: Some(TIGHT_OPTIONS.to_string()),
                    tmax: None,
                    label: "tight",
                }
            }
        }
        RefMode::Tight => RefSpec {
            options: Some(TIGHT_OPTIONS.to_string()),
            tmax: m.ref_tmax,
            label: "tight",
        },
        // literal options replace the default tolerances, the step limit stays tstep/20 unless `bench.ref.tmax`
        RefMode::Options(o) => RefSpec {
            options: Some(o.clone()),
            tmax: if has_tran {
                m.ref_tmax.or(auto_tmax)
            } else {
                m.ref_tmax
            },
            label: "options",
        },
    }
}

// ---------------------------------------------------------------------------------------------------------
// Context and per-case protocol
// ---------------------------------------------------------------------------------------------------------

struct RefEntry {
    plots: Option<Vec<Plot>>,
    status: String,
    options: Option<String>,
    note: Option<String>,
    err: Option<NgSide>,
    /// Peak RSS of the reference run when it was started for this case (`None`: no run, or not measured).
    rss_kib: Option<u64>,
    ran: bool,
}

struct Ctx {
    args: SuiteArgs,
    exe: PathBuf,
    tmp: TmpDir,
    lim: Limits,
    /// Limits for ngspice runs of the current case (shorter when ngspice is only informational).
    ng_lim: Limits,
    ng_installed: bool,
    /// Why ngspice is not usable (`None` when it is).
    ng_reason: Option<String>,
    ref_cache: Option<(String, RefEntry)>,
    artifacts: PathBuf,
    /// Artifacts kept per failure signature (generated fuzz / mutant cases only).
    retained: HashMap<String, usize>,
    /// Bytes of artifacts the run may still keep (starts at [`MAX_ARTIFACT_BYTES`]).
    artifact_budget: u64,
    reps: usize,
}

/// Total artifact bytes a run keeps (Rule 9): beyond it a failing case keeps only its report entry.
const MAX_ARTIFACT_BYTES: u64 = 1 << 30;

/// Write a scratch file. A failing write (full or failing disk, `RLIMIT_FSIZE`) must never be swallowed: both tools
/// would be handed an empty deck and every `loud_error` case would "pass" for the wrong reason.
fn write_file(path: &Path, text: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, text.as_ref())
        .map_err(|e| format!("scratch write failed: cannot write {}: {e}", path.display()))
}

/// A write of a secondary file (log, stderr copy, repro text, artifact): the case verdict does not depend on it, but
/// the loss is recorded once in the notes.
fn note_write(r: &mut CaseResult, res: Result<(), String>) {
    if let Err(e) = res {
        if !r
            .notes
            .iter()
            .any(|n| n.starts_with("scratch write failed"))
        {
            r.notes
                .push(format!("{e}; the artifacts of this case may be incomplete"));
        }
    }
}

/// The case cannot be judged because the harness environment failed: `unverified`, remembered as a harness failure
/// (the run then exits 2, whatever the other cases say).
fn harness_failure(r: &mut CaseResult, msg: String) {
    r.status = Status::Unverified;
    if r.harness_failure.is_none() {
        r.harness_failure = Some(msg.clone());
    }
    r.notes.push(msg);
}

/// A case id as a single, harmless path component (an id taken from a file stem can be `..`).
fn safe_component(id: &str) -> String {
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        "_".to_string()
    } else {
        name
    }
}

/// Scratch directory of a case.
fn case_scratch_dir(ctx: &Ctx, case: &Case) -> PathBuf {
    ctx.tmp.path().join(safe_component(&case.id))
}

/// Removes a case's scratch directory when dropped, on every path out of the case (early returns, panics): a case
/// that was skipped for its >1 GiB raw file used to keep it in RAM-backed `/dev/shm` for the rest of the run.
struct CaseScratch(PathBuf);

impl Drop for CaseScratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base_result(case: &Case) -> CaseResult {
    CaseResult {
        id: case.id.clone(),
        group: case.meta.group.as_str().to_string(),
        family: case.family.clone(),
        origin: case.origin.as_str().to_string(),
        title: case.meta.title.clone(),
        tags: case.meta.tags.clone(),
        expect: case.meta.expect.as_str().to_string(),
        known_gap: case.meta.known_gap.clone(),
        gap_note: case.meta.gap_note.clone(),
        status: Status::Unverified,
        harness_failure: None,
        outcome: None,
        analysis: None,
        size: case.size,
        probes: Vec::new(),
        ngspice_status: "not_run".to_string(),
        ref_status: "not_run".to_string(),
        ref_options: None,
        accuracy: None,
        oracle: None,
        ngspice: None,
        rustspice: None,
        timing: None,
        timing_status: "not_run".to_string(),
        rss: None,
        teeth: None,
        signature: None,
        series: case.series.as_ref().map(|(b, i)| SeriesRef {
            base: b.clone(),
            index: *i,
            tstep: case.tstep,
        }),
        failures: Vec::new(),
        notes: Vec::new(),
        artifacts: None,
        case_wall_ms: 0.0,
    }
}

fn is_budget(msg: &str) -> bool {
    msg.contains("rawfile over budget")
}

fn ref_note_is_budget(ctx: &Ctx) -> bool {
    ctx.ref_cache
        .as_ref()
        .and_then(|(_, e)| e.note.as_deref())
        .is_some_and(is_budget)
}

fn ng_class_msg(c: &NgClass) -> String {
    match c {
        NgClass::Ok => "ok".to_string(),
        NgClass::Error(m) => format!("error: {m}"),
        NgClass::Timeout => "timeout".to_string(),
        NgClass::NotInstalled => "not installed".to_string(),
    }
}

fn ng_side_of(c: &NgClass) -> NgSide {
    match c {
        NgClass::Ok => NgSide::Ok,
        NgClass::Error(m) => NgSide::Error(m.clone()),
        NgClass::Timeout => NgSide::Timeout,
        NgClass::NotInstalled => NgSide::Missing,
    }
}

/// Working directory of a case's tool runs. A corpus deck is fed from the case's scratch directory, never run in
/// place: ngspice drops files such as `b3v33check.log` into its working directory and a benchmark run must not
/// write into the corpus. The scratch `cwd/` holds symbolic links to the deck's siblings that the deck names,
/// directly or through an include chain, so a relative `.include`/`.lib` resolves as it does next to the deck.
/// Generated decks (no source directory) simply run in the scratch directory.
fn case_cwd(case: &Case, dir: &Path) -> PathBuf {
    let Some(src) = case.src_dir.as_deref() else {
        return dir.to_path_buf();
    };
    let cwd = dir.join("cwd");
    if cwd.is_dir() {
        return cwd;
    }
    if std::fs::create_dir_all(&cwd).is_err() {
        return dir.to_path_buf();
    }
    let text = match &case.raw {
        Some(raw) => raw.clone(),
        None => case.deck.clone().into_bytes(),
    };
    for name in referenced_siblings(src, &text) {
        let _ = std::os::unix::fs::symlink(src.join(&name), cwd.join(&name));
    }
    cwd
}

/// Entries of `src` whose name occurs (ASCII case-insensitively) in `text` or in the text of an entry already
/// selected: the closure of the include chain. Over-selecting is harmless, it only adds a link.
fn referenced_siblings(src: &Path, text: &[u8]) -> Vec<std::ffi::OsString> {
    const MAX_SCAN: u64 = 4 << 20;
    const MAX_FILES: usize = 256;
    const MAX_DEPTH: usize = 4;
    let Ok(rd) = std::fs::read_dir(src) else {
        return Vec::new();
    };
    let mut names: Vec<(std::ffi::OsString, String)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let lower = name.to_str()?.to_ascii_lowercase();
            Some((name, lower))
        })
        .collect();
    names.sort();
    let lowered = |b: &[u8]| String::from_utf8_lossy(b).to_ascii_lowercase();
    let mut pending = vec![lowered(text)];
    let mut picked: Vec<std::ffi::OsString> = Vec::new();
    while let Some(hay) = pending.pop() {
        for (name, lower) in &names {
            if picked.contains(name) || !hay.contains(lower.as_str()) {
                continue;
            }
            picked.push(name.clone());
            let mut budget = MAX_FILES;
            for b in file_texts(&src.join(name), MAX_DEPTH, &mut budget, MAX_SCAN) {
                pending.push(lowered(&b));
            }
        }
    }
    picked
}

/// Contents of a file, or of the files below a directory (bounded in depth, count and size).
fn file_texts(p: &Path, depth: usize, budget: &mut usize, max_len: u64) -> Vec<Vec<u8>> {
    let Ok(md) = p.metadata() else {
        return Vec::new();
    };
    if md.is_file() {
        if md.len() > max_len || *budget == 0 {
            return Vec::new();
        }
        *budget -= 1;
        return std::fs::read(p).into_iter().collect();
    }
    let mut out = Vec::new();
    if md.is_dir() && depth > 0 {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                out.extend(file_texts(&e.path(), depth - 1, budget, max_len));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn run_ng(
    ctx: &Ctx,
    case: &Case,
    dir: &Path,
    name: &str,
    text: &[u8],
    solver: NgSolver,
    want_plots: bool,
    r: &mut CaseResult,
) -> Result<ngspice::NgRun, String> {
    let deck = dir.join(format!("{name}.sp"));
    write_file(&deck, text)?;
    let raw = dir.join(format!("{name}.raw"));
    let cwd = case_cwd(case, dir);
    let opts = ngspice::RunOpts {
        cwd: Some(&cwd),
        ignore_log_failures: case.meta.ngspice_error_ok(),
        retry_on_signal: true,
    };
    // a deck that selects KLU itself (`.options klu`) legitimately runs KLU whatever the harness asked for
    let expect_solver = if meta::deck_selects_klu(&String::from_utf8_lossy(text)) {
        None
    } else {
        Some(solver)
    };
    let run = ngspice::run_opts(
        &ctx.args.ngspice,
        &deck,
        &raw,
        &ctx.ng_lim,
        expect_solver,
        want_plots,
        &opts,
    );
    note_write(
        r,
        write_file(
            &dir.join(format!("{name}.log")),
            format!("{}\n{}", run.child.stdout, run.child.stderr),
        ),
    );
    Ok(run)
}

fn run_case(ctx: &mut Ctx, case: &Case) -> CaseResult {
    let t_case = Instant::now();
    let mut r = base_result(case);
    // children of correctness runs die with the harness; the scratch directory is removed on EVERY way out of the
    // case (the same rules for static, family, fuzz and mutant cases)
    let _orphans = rustsim::orphan_guard(true);
    let _scratch = CaseScratch(case_scratch_dir(ctx, case));
    run_case_inner(ctx, case, &mut r);
    r.case_wall_ms = t_case.elapsed().as_secs_f64() * 1e3;
    finalize_rss(&mut r, case.est_rss_mib);
    if ctx.args.teeth && r.teeth.is_none() {
        r.teeth = Some(teeth_not_applicable(r.status, false));
    }
    r
}

/// Why a case without a mutation run has no `teeth` verdict.
fn teeth_not_applicable(status: Status, numeric: bool) -> TeethResult {
    if status != Status::Pass {
        TeethResult::not_applicable(
            "not_a_pass",
            format!(
                "status is {}: only a passing comparison is tested for bluntness",
                status.as_str()
            ),
        )
    } else if !numeric {
        TeethResult::not_applicable(
            "no_numeric_comparison",
            "the pass carries no numeric comparison of waveforms or values (a refusal was expected)"
                .to_string(),
        )
    } else {
        TeethResult::not_applicable(
            "no_reference_data",
            "no waveform data to mutate against".to_string(),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------
// Peak RSS bookkeeping (Rule 9)
// ---------------------------------------------------------------------------------------------------------

/// Record one run of `child` with its measured peak (None = not measured) in the case's RSS report.
fn note_rss(r: &mut CaseResult, child: &str, kib: Option<u64>) {
    let rep = r.rss.get_or_insert_with(|| RssReport {
        children: Vec::new(),
        max_child: None,
        max_kib: None,
        est_mib: None,
        est_status: "unmeasured".to_string(),
        rustspice_kib: None,
        ngspice_max_kib: None,
        ngspice_max_child: None,
    });
    let idx = match rep.children.iter().position(|c| c.child == child) {
        Some(i) => i,
        None => {
            rep.children.push(ChildRss {
                child: child.to_string(),
                peak_kib: None,
                status: "unmeasured".to_string(),
                runs: 0,
            });
            rep.children.len() - 1
        }
    };
    let c = &mut rep.children[idx];
    c.runs += 1;
    if let Some(k) = kib {
        c.peak_kib = Some(c.peak_kib.map_or(k, |x| x.max(k)));
        c.status = "measured_vmhwm".to_string();
    }
}

/// Slack on `bench.est_rss_mib` before a measured peak counts as exceeding it (poll jitter, allocator noise).
const EST_RSS_SLACK: f64 = 1.25;

/// Max over the children, and the comparison of `bench.est_rss_mib` with the RUSTSPICE child only. The estimate is
/// documented (SUITE_SPEC section 2) as RustSpice's peak RSS; ngspice has a start-up floor of 15-20 MiB of its own,
/// which made every small performance deck "exceed" its estimate when the maximum over the children was used. The
/// ngspice peak is reported separately (`ngspice_max_kib`).
fn finalize_rss(r: &mut CaseResult, est_mib: Option<u64>) {
    let Some(rep) = r.rss.as_mut() else { return };
    let top = rep
        .children
        .iter()
        .filter_map(|c| c.peak_kib.map(|k| (c.child.clone(), k)))
        .max_by_key(|(_, k)| *k);
    rep.max_child = top.as_ref().map(|(c, _)| c.clone());
    rep.max_kib = top.as_ref().map(|(_, k)| *k);
    rep.rustspice_kib = rep
        .children
        .iter()
        .find(|c| c.child == "rustspice")
        .and_then(|c| c.peak_kib);
    let ng_top = rep
        .children
        .iter()
        .filter(|c| c.child.starts_with("ngspice"))
        .filter_map(|c| c.peak_kib.map(|k| (c.child.clone(), k)))
        .max_by_key(|(_, k)| *k);
    rep.ngspice_max_child = ng_top.as_ref().map(|(c, _)| c.clone());
    rep.ngspice_max_kib = ng_top.as_ref().map(|(_, k)| *k);
    rep.est_mib = est_mib;
    rep.est_status = match (est_mib, rep.rustspice_kib) {
        (None, _) => "no_estimate",
        (Some(_), None) => "unmeasured",
        (Some(e), Some(k)) => {
            if k as f64 / 1024.0 > e as f64 * EST_RSS_SLACK {
                "exceeds_estimate"
            } else {
                "within_estimate"
            }
        }
    }
    .to_string();
}

/// Result of the RustSpice correctness run (protocol step 3).
struct RsOutcome {
    side: RsSide,
    plots: Option<Vec<Plot>>,
}

/// Run RustSpice once and load its rawfile. `None`: the case ended early (`r.status` and `r.notes` are set).
fn run_rustspice(
    ctx: &Ctx,
    case: &Case,
    dir: &Path,
    deck_path: &Path,
    r: &mut CaseResult,
) -> Option<RsOutcome> {
    let cwd = case_cwd(case, dir);
    let rs_run = match rustsim::run_in(&ctx.exe, deck_path, dir, "rs", &ctx.lim, Some(&cwd)) {
        Ok(x) => x,
        Err(e) => {
            harness_failure(r, format!("cannot start rustspice: {e:?}"));
            return None;
        }
    };
    note_write(r, write_file(&dir.join("rs.stderr"), &rs_run.child.stderr));
    let first_stderr = || {
        rs_run
            .child
            .stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("(empty)")
            .to_string()
    };
    // Outcomes that say nothing about the deck: an environment failure of the harness, or an exit that is neither a
    // success (no readable output) nor a refusal (no message). They end the case `unverified` with the reason; they
    // must never read as a `loud_error` refusal (`expect: loud_error` would "pass" on a full disk).
    let mut anomaly: Option<String> = None;
    if rs_run.infra_failure {
        anomaly = Some(format!(
            "rustspice could not write its output (exit {}): {}; an environment failure (full or failing disk), not a refusal of the deck",
            rustsim::HARNESS_IO_EXIT,
            clip_note(&first_stderr())
        ));
    } else if matches!(rs_run.class, RsClass::LoudError(_)) {
        match rs_run.child.exit {
            rustsim::Exit::Code(0) => {
                anomaly = Some(
                    "rustspice exited 0 but wrote no rawfile: the exit status says success, so this is not a refusal"
                        .to_string(),
                );
            }
            rustsim::Exit::Code(c) if rs_run.child.stderr.trim().is_empty() => {
                anomaly = Some(format!(
                    "rustspice exited {c} with an empty stderr: a refusal must carry a message, this is not judged"
                ));
            }
            _ => {}
        }
    }
    let mut side = match &rs_run.class {
        RsClass::Ok => RsSide::Ok,
        RsClass::LoudError(m) => RsSide::LoudError(m.clone()),
        RsClass::Panic(m) => RsSide::Panic(m.clone()),
        RsClass::Timeout => RsSide::Timeout,
    };
    let mut plots: Option<Vec<Plot>> = None;
    if side == RsSide::Ok {
        match rustsim::load_plots(&rs_run) {
            Ok(p) => plots = Some(p),
            Err(e) if is_budget(&e) => {
                r.status = Status::SkippedBudget;
                r.notes
                    .push(format!("rustspice output exceeds the rawfile budget: {e}"));
                return None;
            }
            Err(e) => {
                // exit 0 and no readable rawfile: not a refusal (the process said it succeeded)
                anomaly = Some(format!(
                    "rustspice exited 0 but its rawfile is unreadable: {e}; this is not a refusal"
                ));
                side = RsSide::LoudError(format!("rustspice wrote an unreadable rawfile: {e}"));
            }
        }
    }
    r.rustspice = Some(RsRunInfo {
        class: match &side {
            RsSide::Ok => "ok",
            RsSide::LoudError(_) => "loud_error",
            RsSide::Panic(_) => "panic",
            RsSide::Timeout => "timeout",
        }
        .to_string(),
        message: match &side {
            RsSide::LoudError(x) | RsSide::Panic(x) => Some(x.clone()),
            _ => None,
        },
        wall_us: Some(rs_run.child.wall_us()),
        rss_kib: rs_run.peak_rss_kib(),
        rss_status: rs_run.rss_status().to_string(),
        rss_poll_kib: rs_run.child.max_rss_kib,
        rss_self_kib: rs_run.stats.as_ref().and_then(|s| s.peak_rss_kib),
        termination: rs_run.termination(),
        stats: rs_run.stats.clone(),
        caught_panic_noise: rs_run.caught_panic_noise,
    });
    note_rss(r, "rustspice", rs_run.peak_rss_kib());
    if rs_run.caught_panic_noise {
        r.notes.push(
            "rustspice printed a caught panic message on stderr but exited cleanly (classification keys on exit code 101 / signals)"
                .to_string(),
        );
    }
    if let Some(a) = anomaly {
        if rs_run.infra_failure {
            harness_failure(r, a);
        } else {
            r.status = Status::Unverified;
            r.notes.push(a);
        }
        return None;
    }
    Some(RsOutcome { side, plots })
}

/// A message cut for a case note.
fn clip_note(s: &str) -> String {
    short_line(s)
}

fn run_case_inner(ctx: &mut Ctx, case: &Case, r: &mut CaseResult) {
    let m = &case.meta;
    r.notes.extend(case.notes.iter().cloned());
    if !case.diags.is_empty() {
        r.status = Status::Unverified;
        r.notes = case.diags.clone();
        r.notes
            .insert(0, "invalid case metadata; not run".to_string());
        return;
    }
    if !ctx.ng_installed {
        r.status = Status::Unverified;
        r.notes.push(match &ctx.ng_reason {
            Some(why) => format!("ngspice is not available: {why}"),
            None => "ngspice is not available".to_string(),
        });
        return;
    }
    if let Some(est) = case.est_rss_mib {
        if est > ctx.args.max_rss_mib {
            r.status = Status::SkippedBudget;
            r.notes.push(format!(
                "estimated RustSpice peak RSS {est} MiB exceeds --max-rss-mib {}",
                ctx.args.max_rss_mib
            ));
            return;
        }
    }
    // ngspice only supplies the pass/fail class for hostile decks: do not wait the full limit for it
    ctx.ng_lim = ctx.lim.clone();
    if m.expect == Expect::LoudError || case.origin == Origin::Mutant {
        ctx.ng_lim.timeout = ctx.lim.timeout.min(Duration::from_secs(10));
    }
    let dir = case_scratch_dir(ctx, case);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        harness_failure(
            r,
            format!("scratch write failed: cannot create {}: {e}", dir.display()),
        );
        return;
    }
    let deck_path = dir.join("deck.sp");
    let wrote = match &case.raw {
        Some(b) => write_file(&deck_path, b),
        None => write_file(&deck_path, &case.deck),
    };
    if let Err(e) = wrote {
        // both tools would be handed an empty deck: never a pass
        harness_failure(r, e);
        return;
    }
    let kinds = detect_kinds(&case.deck);
    let has_tran = kinds.contains(&rawfile::PlotKind::Tran);
    let mut solvers: Vec<NgSolver> = m.ng_solver.solvers().to_vec();
    if solvers.contains(&NgSolver::Sparse) && deck_requests_klu(case) {
        // `ngspice_sparse` cannot be run on a deck that selects KLU itself: the contender would run KLU under the
        // name `ngspice_sparse` and the headline "vs ngspice_sparse" would compare KLU with KLU
        solvers = vec![NgSolver::Klu];
        r.notes.push(
            "the deck selects the KLU solver itself (`.options klu`): ngspice_sparse cannot be run on it, the only ngspice contender is ngspice_klu"
                .to_string(),
        );
    }
    let primary = solvers[0];

    if m.ngspice_unbounded() {
        // ngspice runs away on this deck: no ngspice run, RustSpice is judged against `bench.expect` only
        r.ngspice_status = "skipped_unbounded".to_string();
        r.ref_status = "skipped_unbounded".to_string();
        r.ngspice = Some(NgRunInfo {
            solver: primary.as_str().to_string(),
            status: "skipped_unbounded".to_string(),
            rss_status: "not_run".to_string(),
            ..Default::default()
        });
        r.notes.push(
            "ngspice not run (tag ngspice_unbounded: it never terminates on this deck); judged against bench.expect only"
                .to_string(),
        );
        r.timing_status = "not_requested".to_string();
        let Some(rs) = run_rustspice(ctx, case, &dir, &deck_path, r) else {
            return;
        };
        if !kinds.is_empty() {
            r.analysis = Some(kinds_label(&kinds));
        }
        let mut m2 = m.clone();
        if m2
            .known_gap
            .as_deref()
            .is_some_and(|s| PERF_GAP_SLUGS.contains(&s))
        {
            m2.known_gap = None;
        }
        let verdict = classify(&m2, &NgSide::Skipped, &rs.side, None);
        r.notes.extend(verdict.notes.iter().cloned());
        finish_case(ctx, case, r, &dir, primary, verdict);
        return;
    }

    // series members run ngspice with tmax = tstep, matching RustSpice's fixed grid
    let asis_tmax = if case.series.is_some() {
        case.tstep
    } else {
        None
    };

    // 1. ngspice as-is runs (one per solver)
    let mut asis: Vec<(NgSolver, PathBuf, ngspice::NgRun)> = Vec::new();
    for (k, &sv) in solvers.iter().enumerate() {
        let text = match ng_deck_bytes(case, sv, m.ng_options.as_deref(), asis_tmax) {
            Ok(t) => t,
            Err(e) => {
                r.notes.push(format!("cannot build ngspice deck: {e}"));
                return;
            }
        };
        let name = format!("ng_{}", sv.as_str());
        let run = match run_ng(ctx, case, &dir, &name, &text, sv, k == 0, r) {
            Ok(run) => run,
            Err(e) => {
                harness_failure(r, e);
                return;
            }
        };
        asis.push((sv, dir.join(format!("{name}.sp")), run));
    }
    let asis_primary_ok = asis[0].2.class == NgClass::Ok;
    if asis
        .iter()
        .any(|(_, _, run)| matches!(&run.class, NgClass::Error(m) if is_budget(m)))
    {
        r.status = Status::SkippedBudget;
        r.notes
            .push("ngspice output exceeds the 1 GiB rawfile budget".to_string());
        return;
    }
    r.ngspice_status = ng_class_msg(&asis[0].2.class);
    r.ngspice = Some(NgRunInfo {
        // the solver the log reports (a deck may select KLU itself), else the one the harness asked for
        solver: asis[0].2.log.solver.unwrap_or(primary).as_str().to_string(),
        status: ng_class_msg(&asis[0].2.class),
        wall_us: Some(asis[0].2.child.wall_us()),
        analysis_us: asis[0].2.log.analysis_s.map(|x| x * 1e6),
        // `Total elapsed time` has 1 ms resolution: 0 means "below resolution", reported as null
        elapsed_us: asis[0]
            .2
            .log
            .elapsed_s
            .filter(|x| *x > 0.0)
            .map(|x| x * 1e6),
        rss_kib: asis[0].2.child.max_rss_kib,
        rss_status: if asis[0].2.class == NgClass::NotInstalled {
            "not_run".to_string()
        } else {
            asis[0].2.child.rss_status().to_string()
        },
        rss_lower_bound: asis[0].2.child.max_rss_kib.is_some(),
        output_truncated: asis[0].2.child.capture_incomplete
            || asis[0].2.child.output_dropped_bytes > 0,
    });
    for (sv, _, run) in &asis {
        note_rss(
            r,
            &format!("ngspice_{}", sv.as_str()),
            run.child.max_rss_kib,
        );
    }
    if asis[0].2.class == NgClass::NotInstalled {
        r.status = Status::Unverified;
        r.notes.push("ngspice is not available".to_string());
        return;
    }
    if !asis[0].2.skipped_plots.is_empty() {
        r.notes.push(format!(
            "ngspice also wrote complex plot(s) the suite does not compare: {}",
            asis[0].2.skipped_plots.join(", ")
        ));
    }
    if let Some(n) = &asis[0].2.log_note {
        r.notes.push(n.clone());
    }
    if asis[0].2.attempts > 1 {
        r.notes.push(format!(
            "ngspice crashed on the first attempt and was started once more: {}",
            ng_class_msg(&asis[0].2.class)
        ));
    }
    if !asis[0].2.ignored_failures.is_empty() {
        r.notes.push(format!(
            "ngspice logged `{}` but the deck is tagged ngspice_error_ok (exit 0 and a rawfile count as a run)",
            short_line(&asis[0].2.ignored_failures[0])
        ));
    }
    if m.has_tag("ngspice_crash") && !matches!(asis[0].2.child.exit, rustsim::Exit::Signal(_)) {
        r.notes
            .push("tag ngspice_crash, but ngspice did not crash on this run".to_string());
    }

    // 2. reference run
    let spec = ref_spec(case, has_tran);
    let cache_key = case
        .series
        .as_ref()
        .map(|(b, _)| b.clone())
        .unwrap_or_else(|| case.id.clone());
    let have_cached = ctx
        .ref_cache
        .as_ref()
        .is_some_and(|(k, _)| *k == cache_key && case.series.is_some());
    if !have_cached {
        let entry = if spec.options.is_none() && spec.tmax.is_none() {
            RefEntry {
                plots: None, // filled from the as-is run below
                status: "as_is".to_string(),
                options: None,
                note: None,
                err: None,
                rss_kib: None,
                ran: false,
            }
        } else {
            match ng_deck_bytes(case, primary, spec.options.as_deref(), spec.tmax) {
                Ok(text) => {
                    let mut run = match run_ng(ctx, case, &dir, "ng_ref", &text, primary, true, r) {
                        Ok(run) => run,
                        Err(e) => {
                            r.status = Status::Unverified;
                            r.notes.push(e);
                            return;
                        }
                    };
                    let label = format!(
                        "{}{}",
                        spec.options.clone().unwrap_or_default(),
                        spec.tmax
                            .map_or(String::new(), |t| format!(" tmax={t:.3e}"))
                    );
                    let rss_kib = run.child.max_rss_kib;
                    if run.class == NgClass::Ok {
                        RefEntry {
                            plots: run.plots.as_mut().ok().map(std::mem::take),
                            status: spec.label.to_string(),
                            options: Some(label),
                            note: None,
                            err: None,
                            rss_kib,
                            ran: true,
                        }
                    } else {
                        RefEntry {
                            plots: None,
                            status: "failed".to_string(),
                            options: Some(label),
                            note: Some(format!(
                                "reference run failed: {}",
                                ng_class_msg(&run.class)
                            )),
                            err: Some(ng_side_of(&run.class)),
                            rss_kib,
                            ran: true,
                        }
                    }
                }
                Err(e) => RefEntry {
                    plots: None,
                    status: "failed".to_string(),
                    options: None,
                    note: Some(format!("cannot build reference deck: {e}")),
                    err: Some(NgSide::Error(e)),
                    rss_kib: None,
                    ran: false,
                },
            }
        };
        if entry.ran {
            note_rss(r, "ngspice_ref", entry.rss_kib);
        }
        ctx.ref_cache = Some((cache_key.clone(), entry));
    }
    // resolve reference plots (cache entry stays for the series)
    let asis_plots: Option<&Vec<Plot>> = asis[0].2.plots.as_ref().ok();
    let (ref_plots, ref_status, ref_note, ref_err): (
        Option<&Vec<Plot>>,
        String,
        Option<String>,
        Option<NgSide>,
    ) = {
        let (_, e) = ctx.ref_cache.as_ref().unwrap();
        if e.status == "as_is" {
            (asis_plots, "as_is".to_string(), None, None)
        } else if e.plots.is_some() {
            (e.plots.as_ref(), e.status.clone(), e.note.clone(), None)
        } else if asis_plots.is_some() {
            (
                asis_plots,
                "fell_back".to_string(),
                Some(format!(
                    "{}; falling back to the as-is run",
                    e.note
                        .clone()
                        .unwrap_or_else(|| "reference unavailable".to_string())
                )),
                None,
            )
        } else {
            (None, "failed".to_string(), e.note.clone(), e.err.clone())
        }
    };
    r.ref_status = ref_status.clone();
    r.ref_options = ctx.ref_cache.as_ref().and_then(|(_, e)| e.options.clone());
    if let Some(n) = ref_note {
        r.notes.push(n);
    }
    if !asis_primary_ok && ref_plots.is_some() && ref_status != "as_is" && ref_status != "fell_back"
    {
        r.notes.push(format!(
            "ngspice as-is (default options) failed ({}) but the converged reference solved",
            ng_class_msg(&asis[0].2.class)
        ));
    }
    if ref_note_is_budget(ctx) {
        r.status = Status::SkippedBudget;
        r.notes
            .push("ngspice reference output exceeds the 1 GiB rawfile budget".to_string());
        return;
    }
    let ng_side = if ref_plots.is_some() {
        NgSide::Ok
    } else if let Some(e) = ref_err {
        // prefer the as-is failure class when it is a timeout
        if asis[0].2.class == NgClass::Timeout {
            NgSide::Timeout
        } else {
            e
        }
    } else {
        ng_side_of(&asis[0].2.class)
    };

    // 3. RustSpice correctness run
    let Some(rs) = run_rustspice(ctx, case, &dir, &deck_path, r) else {
        return;
    };

    // 4. accuracy comparison
    let mut cmp: Option<CompareResult> = None;
    let want_cmp = !(case.origin == Origin::Mutant) && m.expect != Expect::LoudError;
    if want_cmp && rs.side == RsSide::Ok && ng_side == NgSide::Ok {
        if let (Some(rsp), Some(rf)) = (rs.plots.as_ref(), ref_plots) {
            let asis_diff = if ref_status != "as_is" && ref_status != "fell_back" {
                asis_plots.map(|v| v.as_slice())
            } else {
                None
            };
            let ng_asis_rows = asis_plots
                .and_then(|p| rawfile::last_plot(p, rawfile::PlotKind::Tran))
                .map(|p| p.npoints);
            cmp = Some(compare_with(
                &CompareInput {
                    meta: m,
                    deck: &case.deck,
                    rs: rsp,
                    reference: rf,
                    asis: asis_diff,
                },
                ng_asis_rows,
            ));
        }
    }
    // the RustSpice waveforms stay alive until the end of the case: `--teeth` compares mutated runs with them
    let rs_plots = rs.plots;

    // 5. classification
    let perf_gap = m
        .known_gap
        .as_deref()
        .is_some_and(|s| PERF_GAP_SLUGS.contains(&s));
    let mut m2 = m.clone();
    if perf_gap {
        m2.known_gap = None;
    }
    let mut verdict = classify(&m2, &ng_side, &rs.side, cmp.as_ref().and_then(|c| c.pass));
    downgrade_coarse_series_member(case, &mut verdict);
    demote_on_fallback_reference(&ref_status, cmp.as_ref().and_then(|c| c.pass), &mut verdict);
    if verdict.status == Status::Unverified
        && m.has_tag("lenient_ok")
        && cmp.as_ref().is_some_and(|c| c.vacuous_only)
    {
        verdict.status = Status::Info;
        verdict.notes.push(
            "lenient_ok: every probe is vacuous (reference ~0) and rustspice agrees; informational"
                .to_string(),
        );
    }
    r.notes.extend(verdict.notes.clone());
    if let Some(c) = &cmp {
        r.analysis = Some(c.analysis.clone());
        r.probes = c.probes.clone();
        r.failures = c.failures.clone();
        r.notes.extend(c.notes.clone());
        r.oracle = c.oracle.clone();
        if let Some(o) = &c.oracle {
            r.notes.extend(oracle_notes(o, m));
        }
        r.accuracy = Some(AccuracySummary {
            pass: c.pass,
            err_rs: c.err_rs,
            err_ng: c.err_ng,
            within_ng_error: c.within_ng_error,
            interp_floor: c.interp_floor,
            points_rs: c.points_rs,
            points_ref: c.points_ref,
        });
    } else if !kinds.is_empty() {
        r.analysis = Some(kinds_label(&kinds));
    }
    if matches!(verdict.status, Status::Pass | Status::Info) {
        // an accuracy pass is only a pass when nothing was unverifiable; keep ngspice failure of as-is visible
        if !asis_primary_ok && m.timing {
            r.notes
                .push("timing skipped: ngspice as-is run failed".to_string());
        }
    }

    // 6. timing
    let timing_wanted = m.timing && !ctx.args.no_timing;
    if !m.timing {
        r.timing_status = "not_requested".to_string();
    } else if ctx.args.no_timing {
        r.timing_status = "disabled".to_string();
    }
    if timing_wanted {
        if !matches!(verdict.status, Status::Pass | Status::Info) {
            r.timing_status = "skipped_status".to_string();
        } else if rs.side != RsSide::Ok {
            r.timing_status = "rustspice_failed".to_string();
        } else if !asis.iter().all(|(_, _, run)| run.class == NgClass::Ok) {
            r.timing_status = "ngspice_failed".to_string();
        } else {
            let decks: Vec<(NgSolver, PathBuf)> =
                asis.iter().map(|(s, p, _)| (*s, p.clone())).collect();
            let cwd = case_cwd(case, &dir);
            let t = time_case(ctx, &dir, &decks, &deck_path, &cwd, m.ngspice_error_ok());
            r.timing_status = timing_status_of(&t).to_string();
            for c in &t.contenders {
                let peak = c
                    .rss_kib
                    .as_ref()
                    .map(|x| x.samples.iter().cloned().fold(0.0, f64::max) as u64);
                note_rss(r, &c.name, peak);
            }
            r.timing = Some(t);
        }
    }

    // performance-only known gaps are judged on the measured END-TO-END speedup (SUITE_SPEC 14.3)
    if perf_gap {
        let (st, notes) = judge_perf_gap(
            verdict.status,
            r.timing.as_ref(),
            &r.timing_status,
            m.known_gap.as_deref().unwrap_or(""),
        );
        verdict.status = st;
        r.notes.extend(notes);
    }

    // `--teeth`: mutation sensitivity of a numeric pass (after the final status is known)
    if ctx.args.teeth {
        let numeric = cmp.as_ref().and_then(|c| c.pass) == Some(true);
        r.teeth = Some(
            match (verdict.status, numeric, rs_plots.as_ref(), ref_plots) {
                (Status::Pass, true, Some(rsp), Some(rf)) => run_teeth(
                    ctx,
                    case,
                    &dir,
                    rf,
                    rsp,
                    r.rustspice.as_ref().and_then(|x| x.wall_us),
                    r.probes
                        .iter()
                        .filter(|p| !p.vacuous)
                        .filter_map(|p| p.tol_ratio)
                        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v)))),
                ),
                (status, _, _, _) => teeth_not_applicable(status, numeric),
            },
        );
    }
    finish_case(ctx, case, r, &dir, primary, verdict);
}

/// Speedup (median end-to-end versus the faster ngspice solver) from which a performance gap counts as closed.
pub const PERF_CLOSED_SPEEDUP: f64 = 1.1;

/// Successful end-to-end samples every contender needs before a timing is called `measured` or judged.
pub const MIN_TIMING_SAMPLES: usize = 3;

/// `measured` only when EVERY contender produced at least [`MIN_TIMING_SAMPLES`] successful end-to-end samples
/// (failed repetitions are not samples); otherwise `unstable`.
fn timing_status_of(t: &TimingResult) -> &'static str {
    if !t.contenders.is_empty() && t.min_samples >= MIN_TIMING_SAMPLES {
        "measured"
    } else {
        "unstable"
    }
}

/// Verdict of a case whose `known_gap` is a performance slug (`PERF_GAP_SLUGS`). `accuracy` is the status the
/// comparison alone produced. The measure is the END-TO-END median speedup (the user-visible number); the
/// solve-only ratio is reported in the note, never judged on. `gap_closed` needs a speedup of at least
/// [`PERF_CLOSED_SPEEDUP`] AND timing that is not noisy; slower, in the parity band or noisy keeps the gap.
/// Without timing nothing can be said (`unverified`). A failure of the comparison is NOT covered by a
/// performance slug and keeps its `fail_*` status, except a timeout (RustSpice slower than the whole watchdog
/// limit, the extreme of "slow").
pub fn judge_perf_gap(
    accuracy: Status,
    timing: Option<&TimingResult>,
    timing_status: &str,
    slug: &str,
) -> (Status, Vec<String>) {
    match accuracy {
        Status::Pass | Status::Info => {}
        Status::Timeout => {
            return (
                Status::KnownGapConfirmed,
                vec![format!(
                    "performance gap `{slug}` confirmed: rustspice hit the wall-clock limit where ngspice finished"
                )],
            )
        }
        s if s.is_fail() => {
            return (
                s,
                vec![format!(
                    "known_gap `{slug}` is performance-only and does not cover this {}: the failure stays visible",
                    s.as_str()
                )],
            )
        }
        other => return (other, Vec::new()),
    }
    let Some(t) = timing else {
        return (
            Status::Unverified,
            vec![format!(
                "performance known_gap `{slug}` not judged: no end-to-end timing was measured (timing_status `{timing_status}`)"
            )],
        );
    };
    // every contender needs enough SUCCESSFUL samples: a speedup from one ngspice sample (the other repetitions
    // failed) or from a missing solver is no measurement
    let thin: Vec<String> = t
        .contenders
        .iter()
        .filter_map(|c| {
            let n = c.e2e_us.as_ref().map_or(0, |x| x.n);
            (n < MIN_TIMING_SAMPLES).then(|| format!("{} {n}", c.name))
        })
        .collect();
    if !t.contenders.is_empty() && !thin.is_empty() {
        return (
            Status::Unverified,
            vec![format!(
                "performance known_gap `{slug}` not judged: too few successful end-to-end samples ({}; at least {MIN_TIMING_SAMPLES} per contender are needed; timing_status `{timing_status}`)",
                thin.join(", ")
            )],
        );
    }
    let has_rs = t.contenders.iter().any(|c| c.name == "rustspice");
    let has_ng = t.contenders.iter().any(|c| c.name.starts_with("ngspice_"));
    if !t.contenders.is_empty() && !(has_rs && has_ng) {
        return (
            Status::Unverified,
            vec![format!(
                "performance known_gap `{slug}` not judged: the timing lacks a contender (rustspice present: {has_rs}, ngspice present: {has_ng}; timing_status `{timing_status}`)"
            )],
        );
    }
    let Some(x) = t.headline.e2e_median else {
        return (
            Status::Unverified,
            vec![format!(
                "performance known_gap `{slug}` not judged: the timed repetitions produced no end-to-end speedup (timing_status `{timing_status}`)"
            )],
        );
    };
    let vs = t.headline.e2e_vs.clone().unwrap_or_default();
    let solve = t
        .headline
        .solve_median
        .map_or("n/a".to_string(), |v| format!("{v:.2}x"));
    let detail = format!(
        "end-to-end median speedup {x:.2}x versus {vs} (solve-only {solve}; the solve-only ratio is informational)"
    );
    if x < 1.0 {
        (
            Status::KnownGapConfirmed,
            vec![format!(
                "performance gap `{slug}` confirmed: rustspice is slower, {detail}"
            )],
        )
    } else if x >= PERF_CLOSED_SPEEDUP && !t.noisy {
        (
            Status::GapClosed,
            vec![format!(
                "performance gap `{slug}` closed: rustspice is faster, {detail}"
            )],
        )
    } else if x >= PERF_CLOSED_SPEEDUP {
        (
            Status::KnownGapConfirmed,
            vec![format!(
                "performance gap `{slug}` not called closed: {detail}, but the timing is noisy (re-measure on a quiet host)"
            )],
        )
    } else {
        (
            Status::KnownGapConfirmed,
            vec![format!(
                "performance gap `{slug}` not called closed: {detail} is below the {PERF_CLOSED_SPEEDUP:.1}x closing threshold"
            )],
        )
    }
}

/// Work-precision series: the tolerance belongs to the deck's own `.tran` step. A member coarser than that step
/// that misses it is a measurement of the convergence curve (its error goes to the series table), so it is
/// reported as `info` instead of `fail_accuracy`. A member with a `known_gap` keeps its gap status.
fn downgrade_coarse_series_member(case: &Case, verdict: &mut compare::Verdict) {
    if let (Some(nom), Some(h)) = (case.nominal_tstep, case.tstep) {
        if h > nom * (1.0 + 1e-9) && verdict.status == Status::FailAccuracy {
            verdict.status = Status::Info;
            verdict.notes.push(format!(
                "series member coarser than the nominal .tran step ({nom:.3e} s): tol.norm applies from the nominal step down; this point only feeds the work-precision table"
            ));
        }
    }
}

/// A numeric violation measured against a `fell_back` reference (the converged reference run failed and the as-is,
/// default-tolerance ngspice run stood in for it) is not evidence about RustSpice: it may be ngspice's own error.
/// `fail_accuracy` and `known_gap_confirmed` become `unverified` with the reason; every other outcome (a refusal, a
/// panic, a pass) does not depend on the reference and is left alone.
fn demote_on_fallback_reference(
    ref_status: &str,
    cmp_pass: Option<bool>,
    verdict: &mut compare::Verdict,
) {
    if ref_status == "fell_back"
        && cmp_pass == Some(false)
        && matches!(
            verdict.status,
            Status::FailAccuracy | Status::KnownGapConfirmed
        )
    {
        let was = verdict.status;
        verdict.status = Status::Unverified;
        verdict.notes.push(format!(
            "{} withdrawn: the converged reference run failed and the as-is (default tolerance) ngspice run stands in for it (ref_status `fell_back`), so the numeric violation may be ngspice's own error; not judged",
            was.as_str()
        ));
    }
}

/// Notes about the independent oracle of a case: an oracle that could not run, or that disagrees with BOTH
/// simulators (its spec or the deck is then suspect), must not pass without a word.
fn oracle_notes(o: &compare::OracleSummary, m: &Meta) -> Vec<String> {
    let mut v = Vec::new();
    if o.status != "ok" {
        v.push(format!(
            "oracle `{}` was not evaluated (status `{}`{}): the case has no independent truth",
            o.spec,
            o.status,
            o.reason
                .as_deref()
                .map_or(String::new(), |r| format!(": {}", short_line(r)))
        ));
        return v;
    }
    let tol = if o.metric == "norm" {
        m.tol_norm
    } else {
        m.tol_abs + m.tol_rel
    };
    if let (Some(rs), Some(ng)) = (o.rs_max_err, o.ng_max_err) {
        if rs > 10.0 * tol && ng > 10.0 * tol {
            v.push(format!(
                "oracle `{}` disagrees with BOTH simulators (max error rustspice {rs:.3e}, ngspice {ng:.3e}, metric `{}`, more than 10x the case tolerance {tol:.1e}): the oracle spec or the deck is suspect, this is not a verdict on either tool",
                o.spec, o.metric
            ));
        }
    }
    v
}

/// Whether the deck (or its `bench.ng.options`) asks for ngspice's KLU solver itself.
fn deck_requests_klu(case: &Case) -> bool {
    meta::deck_selects_klu(&case.deck)
        || case
            .meta
            .ng_options
            .as_deref()
            .is_some_and(|o| o.split_whitespace().any(|w| w.eq_ignore_ascii_case("klu")))
}

fn kinds_label(kinds: &[rawfile::PlotKind]) -> String {
    kinds
        .iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()
        .join("+")
}

fn short_line(s: &str) -> String {
    let t: String = s.chars().take(100).collect();
    t
}

/// Artifacts kept per distinct failure signature of generated fuzz / mutant cases (SUITE_SPEC 14.3): a common root
/// cause must not use up the budget that would otherwise show a different defect.
pub const ARTIFACTS_PER_SIGNATURE: usize = 3;
/// Hard cap on distinct signatures that get their own artifact bucket (Rule 9); the rest share one bucket.
pub const MAX_SIGNATURES: usize = 64;

/// Shape of a message line: lower case, every number (and hex literal) replaced by `#`, whitespace collapsed,
/// at most 110 characters. Two failures with the same shape have the same root cause for triage purposes.
pub fn line_shape(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            // a whole number token: digits, dots, exponent with its sign, hex digits after 0x
            let mut prev = c;
            while let Some(&n) = chars.peek() {
                let exp_sign = matches!(n, '+' | '-') && matches!(prev, 'e' | 'E');
                if n.is_ascii_alphanumeric() || n == '.' || n == '_' || exp_sign {
                    prev = n;
                    chars.next();
                } else {
                    break;
                }
            }
            out.push('#');
        } else if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c.to_ascii_lowercase());
        }
        if out.len() >= 110 {
            break;
        }
    }
    out
}

/// `status|shape of the first stderr line` of RustSpice (else of the first failure or note).
fn failure_signature(r: &CaseResult, dir: &Path) -> String {
    let first = std::fs::read_to_string(dir.join("rs.stderr"))
        .ok()
        .and_then(|t| {
            t.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(String::from)
        })
        .or_else(|| r.failures.first().cloned())
        .or_else(|| r.notes.first().cloned())
        .unwrap_or_default();
    format!("{}|{}", r.status.as_str(), line_shape(&first))
}

/// Record the verdict, keep the artifacts of a non-passing case (protocol step 7) and remove the scratch dir.
fn finish_case(
    ctx: &mut Ctx,
    case: &Case,
    r: &mut CaseResult,
    dir: &Path,
    primary: NgSolver,
    verdict: compare::Verdict,
) {
    r.status = verdict.status;
    r.outcome = verdict.outcome.map(|o| o.as_str().to_string());
    let generated = matches!(case.origin, Origin::Fuzz | Origin::Mutant);
    let keep = match r.status {
        Status::FailAccuracy
        | Status::FailRobustness
        | Status::FailSilent
        | Status::Panic
        | Status::Timeout => true,
        // every other non-pass case keeps its artifacts too (generated decks only when they fail)
        Status::KnownGapConfirmed
        | Status::GapClosed
        | Status::BothError
        | Status::NgspiceError
        | Status::Unverified
        | Status::Info => !generated,
        Status::Pass | Status::SkippedBudget => false,
    };
    if keep {
        let mut capped = false;
        if generated {
            let sig = failure_signature(r, dir);
            r.signature = Some(sig.clone());
            let key = if ctx.retained.contains_key(&sig) || ctx.retained.len() < MAX_SIGNATURES {
                sig
            } else {
                "(other signatures)".to_string()
            };
            let n = ctx.retained.entry(key).or_default();
            capped = *n >= ARTIFACTS_PER_SIGNATURE;
            if !capped {
                *n += 1;
            }
        }
        if !capped {
            let repro = format!(
                "cd {}\nngspice -b -r ng.raw ng_{}.sp\n{} run deck.sp --raw rs.raw --stats rs.json\n",
                case.src_dir.as_deref().unwrap_or(dir).display(),
                primary.as_str(),
                ctx.exe.display()
            );
            note_write(r, write_file(&dir.join("repro.txt"), repro));
            let (kept, problems) = copy_artifacts(
                dir,
                &ctx.artifacts.join(safe_component(&case.id)),
                &mut ctx.artifact_budget,
            );
            r.artifacts = kept;
            if !problems.is_empty() {
                r.notes
                    .push(format!("artifacts incomplete: {}", problems.join("; ")));
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Copy the files of a scratch directory (each up to 64 MiB, all artifacts of the run up to [`MAX_ARTIFACT_BYTES`])
/// to `dst`. Returns the directory when it exists and the problems (a copy that failed, a file left out for size),
/// never silently dropping a file.
fn copy_artifacts(src: &Path, dst: &Path, budget: &mut u64) -> (Option<String>, Vec<String>) {
    let mut problems = Vec::new();
    if let Err(e) = std::fs::create_dir_all(dst) {
        return (None, vec![format!("cannot create {}: {e}", dst.display())]);
    }
    let Ok(rd) = std::fs::read_dir(src) else {
        return (
            Some(dst.display().to_string()),
            vec![format!("cannot read {}", src.display())],
        );
    };
    for e in rd.flatten() {
        let p = e.path();
        if let Ok(md) = p.metadata() {
            if !md.is_file() {
                continue;
            }
            if md.len() > 64 << 20 {
                problems.push(format!(
                    "{} not kept ({} MiB > 64 MiB)",
                    e.file_name().to_string_lossy(),
                    md.len() >> 20
                ));
            } else if md.len() > *budget {
                problems.push(format!(
                    "{} not kept (the run's artifact budget of {} MiB is used up)",
                    e.file_name().to_string_lossy(),
                    MAX_ARTIFACT_BYTES >> 20
                ));
            } else {
                match std::fs::copy(&p, dst.join(e.file_name())) {
                    Ok(n) => *budget -= n.min(*budget),
                    Err(err) => {
                        problems.push(format!("{}: {err}", e.file_name().to_string_lossy()))
                    }
                }
            }
        }
    }
    (Some(dst.display().to_string()), problems)
}

// ---------------------------------------------------------------------------------------------------------
// `--teeth`: mutation sensitivity of a passing comparison
// ---------------------------------------------------------------------------------------------------------

/// Original RustSpice runs longer than this fraction of `--case-timeout` are not mutated (three more runs of the
/// same size would eat the budget of the whole suite).
const TEETH_MAX_RUN_FRACTION: f64 = 0.25;

/// Run RustSpice alone on the mutated decks of one passing case and judge whether the comparison (against the
/// ORIGINAL ngspice `reference`, with the case's own tolerances) can see them.
fn run_teeth(
    ctx: &Ctx,
    case: &Case,
    dir: &Path,
    reference: &[Plot],
    orig_rs: &[Plot],
    orig_wall_us: Option<f64>,
    baseline_tol_ratio: Option<f64>,
) -> TeethResult {
    if case.raw.is_some() {
        return TeethResult::skipped(
            "non_utf8_deck",
            "the deck is not valid UTF-8; its text cannot be edited safely".to_string(),
        );
    }
    if let Some(w) = orig_wall_us {
        let limit = ctx.args.case_timeout as f64 * 1e6 * TEETH_MAX_RUN_FRACTION;
        if w > limit {
            return TeethResult::skipped(
                "run_too_long",
                format!(
                    "the unmutated RustSpice run took {:.1} s (more than {:.0}% of --case-timeout); three more runs are not affordable",
                    w / 1e6,
                    TEETH_MAX_RUN_FRACTION * 100.0
                ),
            );
        }
    }
    let kinds = detect_kinds(&case.deck);
    let sc = teeth::scan(&case.deck, kinds.contains(&rawfile::PlotKind::Tran));
    if sc.candidates.is_empty() {
        let why = sc.skipped_summary();
        return TeethResult::skipped(
            "no_mutable_element",
            if why.is_empty() {
                "the deck has no R/C/L/V/I/G/MOS card".to_string()
            } else {
                format!("no scalable element ({why})")
            },
        );
    }
    // the oracle describes the unmutated circuit; it has no meaning for a mutant
    let mut meta = case.meta.clone();
    meta.oracle = None;
    let cwd = case_cwd(case, dir);
    let mut muts: Vec<MutationResult> = Vec::new();
    for (k, &ci) in teeth::pick(sc.candidates.len()).iter().enumerate() {
        let c = &sc.candidates[ci];
        let text = teeth::apply(&case.deck, c, teeth::FACTOR);
        let deck_path = dir.join(format!("teeth_{k}.sp"));
        if let Err(e) = write_file(&deck_path, &text) {
            // a mutant that was never run must not count as "detected" (the old harness ran a missing deck, got a
            // loud error and called the mutation detected)
            muts.push(MutationResult {
                element: c.name.clone(),
                line: c.line + 1,
                kind: c.kind.to_string(),
                param: c.param.clone(),
                from: c.text.clone(),
                to: teeth::mutated_text(c, teeth::FACTOR),
                outcome: "unevaluable".to_string(),
                rustspice: "spawn_failed".to_string(),
                worst_tol_ratio: None,
                response: None,
                note: Some(e),
            });
            continue;
        }
        muts.push(one_mutation(
            ctx,
            case,
            dir,
            &deck_path,
            &format!("teeth_{k}"),
            c,
            &meta,
            reference,
            orig_rs,
            &cwd,
        ));
    }
    let v = teeth::verdict(&muts);
    let mut res = TeethResult {
        status: v.status,
        reason_class: v.reason_class,
        reason: v.reason,
        factor: teeth::FACTOR,
        baseline_tol_ratio,
        detected_by: v.detected_by,
        sources_only: v.sources_only,
        weak_reason_class: v.weak_reason_class,
        mutations: muts,
        artifacts: None,
    };
    if res.status == "blunt" {
        // keep the mutated decks next to the other artifacts so a deck owner can re-run them
        let dst = ctx.artifacts.join(safe_component(&case.id));
        if std::fs::create_dir_all(&dst).is_ok() {
            for k in 0..res.mutations.len() {
                let _ = std::fs::copy(
                    dir.join(format!("teeth_{k}.sp")),
                    dst.join(format!("teeth_{k}.sp")),
                );
            }
            let lines: Vec<String> = res
                .mutations
                .iter()
                .enumerate()
                .map(|(k, m)| {
                    format!(
                        "teeth_{k}.sp: {} {} {} -> {} ({}, response {}, tol ratio {})",
                        m.element,
                        m.param,
                        m.from,
                        m.to,
                        m.outcome,
                        m.response.map_or("n/a".to_string(), |x| format!("{x:.2e}")),
                        m.worst_tol_ratio
                            .map_or("n/a".to_string(), |x| format!("{x:.2e}"))
                    )
                })
                .collect();
            let _ = write_file(&dst.join("teeth.txt"), lines.join("\n") + "\n");
            res.artifacts = Some(dst.display().to_string());
        }
    }
    res
}

#[allow(clippy::too_many_arguments)]
fn one_mutation(
    ctx: &Ctx,
    case: &Case,
    dir: &Path,
    deck_path: &Path,
    tag: &str,
    c: &teeth::Candidate,
    meta: &Meta,
    reference: &[Plot],
    orig_rs: &[Plot],
    cwd: &Path,
) -> MutationResult {
    let mut res = MutationResult {
        element: c.name.clone(),
        line: c.line + 1,
        kind: c.kind.to_string(),
        param: c.param.clone(),
        from: c.text.clone(),
        to: teeth::mutated_text(c, teeth::FACTOR),
        outcome: "unevaluable".to_string(),
        rustspice: String::new(),
        worst_tol_ratio: None,
        response: None,
        note: None,
    };
    let run = match rustsim::run_in(&ctx.exe, deck_path, dir, tag, &ctx.lim, Some(cwd)) {
        Ok(r) => r,
        Err(e) => {
            res.rustspice = "spawn_failed".to_string();
            res.note = Some(format!("cannot start rustspice: {e:?}"));
            return res;
        }
    };
    match &run.class {
        RsClass::Ok => res.rustspice = "ok".to_string(),
        RsClass::LoudError(m) => {
            res.rustspice = "loud_error".to_string();
            res.note = Some(m.clone());
            return res;
        }
        RsClass::Panic(m) => {
            res.rustspice = "panic".to_string();
            res.note = Some(m.clone());
            return res;
        }
        RsClass::Timeout => {
            res.rustspice = "timeout".to_string();
            return res;
        }
    }
    let plots = match rustsim::load_plots(&run) {
        Ok(p) => p,
        Err(e) => {
            res.rustspice = "unreadable_output".to_string();
            res.note = Some(e);
            return res;
        }
    };
    // (a) the verdict of the comparison on the mutant, against the ORIGINAL ngspice data
    let vs_ref = compare_with(
        &CompareInput {
            meta,
            deck: &case.deck,
            rs: &plots,
            reference,
            asis: None,
        },
        None,
    );
    res.worst_tol_ratio = vs_ref
        .probes
        .iter()
        .filter(|p| !p.vacuous)
        .filter_map(|p| p.tol_ratio)
        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))));
    res.outcome = match vs_ref.pass {
        Some(true) => "survived",
        Some(false) => "detected",
        None => {
            res.note = Some("the comparison of the mutated run could not be evaluated".to_string());
            "unevaluable"
        }
    }
    .to_string();
    // (b) how far the mutation moved the probes (mutated vs unmutated RustSpice, same metric)
    let vs_self = compare_with(
        &CompareInput {
            meta,
            deck: &case.deck,
            rs: &plots,
            reference: orig_rs,
            asis: None,
        },
        None,
    );
    res.response = vs_self
        .probes
        .iter()
        .filter(|p| !p.vacuous)
        .filter_map(|p| {
            if p.analysis == "tran" {
                p.norm_err
            } else {
                p.rel_err
            }
        })
        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))));
    res
}

// ---------------------------------------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Samples {
    e2e: Vec<f64>,
    analysis: Vec<f64>,
    setup: Vec<f64>,
    overhead: Vec<f64>,
    parse: Vec<f64>,
    compile: Vec<f64>,
    rss: Vec<f64>,
    failed: usize,
}

#[derive(Clone)]
enum Contender {
    Ng(NgSolver, PathBuf),
    Rs,
}

impl Contender {
    fn name(&self) -> String {
        match self {
            Contender::Ng(s, _) => format!("ngspice_{}", s.as_str()),
            Contender::Rs => "rustspice".to_string(),
        }
    }
}

/// One timed run of a contender; returns `false` if it failed. `ng_ignore_log_failures` is the deck's
/// `ngspice_error_ok` tag: the correctness run counted such a log as a run, so the timed repetitions must too.
fn timed_run(
    ctx: &Ctx,
    c: &Contender,
    dir: &Path,
    rs_deck: &Path,
    cwd: &Path,
    s: &mut Samples,
    ng_ignore_log_failures: bool,
) -> bool {
    // a timed repetition is started with `posix_spawn`: the `fork` + PDEATHSIG path costs 0.4-1.3 ms per start
    let _no_orphan_guard = rustsim::orphan_guard(false);
    match c {
        Contender::Ng(sv, deck) => {
            let raw = dir.join(format!("t_{}.raw", sv.as_str()));
            let opts = ngspice::RunOpts {
                cwd: Some(cwd),
                ignore_log_failures: ng_ignore_log_failures,
                // a non-deterministic crash costs the repetition its sample, not the whole timing
                retry_on_signal: true,
            };
            let run = ngspice::run_opts(
                &ctx.args.ngspice,
                deck,
                &raw,
                &ctx.lim,
                Some(*sv),
                false,
                &opts,
            );
            if run.class != NgClass::Ok {
                s.failed += 1;
                return false;
            }
            s.e2e.push(run.child.wall_us());
            if let Some(a) = run.log.analysis_s {
                s.analysis.push(a * 1e6);
                s.overhead.push((run.child.wall_us() - a * 1e6).max(0.0));
                // elapsed has 1 ms resolution: only meaningful when it is at least the analysis time
                if let Some(e) = run.log.elapsed_s.filter(|e| *e > 0.0 && *e >= a) {
                    s.setup.push((e - a) * 1e6);
                }
            }
            if let Some(k) = run.child.max_rss_kib {
                s.rss.push(k as f64);
            }
            true
        }
        Contender::Rs => {
            let run = match rustsim::run_in(&ctx.exe, rs_deck, dir, "t_rs", &ctx.lim, Some(cwd)) {
                Ok(r) => r,
                Err(_) => {
                    s.failed += 1;
                    return false;
                }
            };
            if run.class != RsClass::Ok {
                s.failed += 1;
                return false;
            }
            s.e2e.push(run.child.wall_us());
            if let Some(st) = &run.stats {
                if let Some(a) = st.analysis_us {
                    s.analysis.push(a);
                    s.overhead.push((run.child.wall_us() - a).max(0.0));
                }
                if let Some(p) = st.parse_us {
                    s.parse.push(p);
                }
                if let Some(cp) = st.compile_us {
                    s.compile.push(cp);
                }
                if let (Some(p), Some(cp)) = (st.parse_us, st.compile_us) {
                    s.setup.push(p + cp);
                }
            }
            if let Some(k) = run.peak_rss_kib() {
                s.rss.push(k as f64);
            }
            true
        }
    }
}

fn contender_timing(name: String, s: &Samples) -> ContenderTiming {
    let rss_status = if s.rss.is_empty() {
        "unmeasured"
    } else if s.rss.len() == s.e2e.len() {
        "measured_vmhwm"
    } else {
        "partial"
    };
    ContenderTiming {
        rss_status: rss_status.to_string(),
        name,
        e2e_us: summarize(&s.e2e),
        analysis_us: summarize(&s.analysis),
        setup_us: summarize(&s.setup),
        overhead_us: summarize(&s.overhead),
        parse_us: summarize(&s.parse),
        compile_us: summarize(&s.compile),
        rss_kib: summarize(&s.rss),
        failed_reps: s.failed,
    }
}

fn ratio(num: Option<f64>, den: Option<f64>) -> Option<f64> {
    match (num, den) {
        (Some(a), Some(b)) if a.is_finite() && b.is_finite() && b > 0.0 => Some(a / b),
        _ => None,
    }
}

/// Speedups `ngspice / rustspice` against the fastest ngspice solver (per metric).
pub fn headline(cs: &[ContenderTiming]) -> Headline {
    let rs = cs.iter().find(|c| c.name == "rustspice");
    let ngs: Vec<&ContenderTiming> = cs
        .iter()
        .filter(|c| c.name.starts_with("ngspice_"))
        .collect();
    let mut h = Headline::default();
    let pick = |f: &dyn Fn(&ContenderTiming) -> Option<f64>| -> Option<(String, f64)> {
        ngs.iter()
            .filter_map(|c| f(c).map(|v| (c.name.clone(), v)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    };
    let e2e_med = |c: &ContenderTiming| c.e2e_us.as_ref().map(|s| s.median);
    let e2e_min = |c: &ContenderTiming| c.e2e_us.as_ref().map(|s| s.min);
    let sol_med = |c: &ContenderTiming| c.analysis_us.as_ref().map(|s| s.median);
    let sol_min = |c: &ContenderTiming| c.analysis_us.as_ref().map(|s| s.min);
    if let Some(rs) = rs {
        if let Some((n, v)) = pick(&e2e_med) {
            h.e2e_median = ratio(Some(v), e2e_med(rs));
            h.e2e_vs = Some(n);
        }
        if let Some((_, v)) = pick(&e2e_min) {
            h.e2e_min = ratio(Some(v), e2e_min(rs));
        }
        if let Some((n, v)) = pick(&sol_med) {
            h.solve_median = ratio(Some(v), sol_med(rs));
            h.solve_vs = Some(n);
        }
        if let Some((_, v)) = pick(&sol_min) {
            h.solve_min = ratio(Some(v), sol_min(rs));
        }
        for c in &ngs {
            h.per_solver.push(SolverSpeedup {
                solver: c.name.clone(),
                e2e_median: ratio(e2e_med(c), e2e_med(rs)),
                e2e_min: ratio(e2e_min(c), e2e_min(rs)),
                solve_median: ratio(sol_med(c), sol_med(rs)),
                solve_min: ratio(sol_min(c), sol_min(rs)),
            });
        }
    }
    h
}

fn time_case(
    ctx: &Ctx,
    dir: &Path,
    ng_decks: &[(NgSolver, PathBuf)],
    rs_deck: &Path,
    cwd: &Path,
    ng_ignore_log_failures: bool,
) -> TimingResult {
    let mut contenders: Vec<Contender> = ng_decks
        .iter()
        .map(|(s, p)| Contender::Ng(*s, p.clone()))
        .collect();
    contenders.push(Contender::Rs);
    let n = contenders.len();
    let mut samples: Vec<Samples> = (0..n).map(|_| Samples::default()).collect();
    let budget = Duration::from_secs(ctx.args.case_timeout.saturating_mul(2).max(10));
    let t0 = Instant::now();
    let mut t = TimingResult {
        reps_requested: ctx.reps,
        loadavg_before: rustsim::loadavg(),
        ..Default::default()
    };
    for rep in 0..ctx.reps {
        // rotate the execution order every repetition: with two contenders this alternates A-first / B-first
        let order: Vec<usize> = (0..n).map(|i| (i + rep) % n).collect();
        t.order_trace.push(
            order
                .iter()
                .map(|&i| contenders[i].name())
                .collect::<Vec<_>>()
                .join(">"),
        );
        for &i in &order {
            timed_run(
                ctx,
                &contenders[i],
                dir,
                rs_deck,
                cwd,
                &mut samples[i],
                ng_ignore_log_failures,
            );
        }
        t.reps_attempted += 1;
        if t0.elapsed() > budget && t.reps_attempted >= 3 {
            t.notes.push(format!(
                "timing truncated after {} reps: per-case timing budget {} s exhausted",
                t.reps_attempted,
                budget.as_secs()
            ));
            break;
        }
    }
    t.loadavg_after = rustsim::loadavg();
    t.contenders = contenders
        .iter()
        .zip(samples.iter())
        .map(|(c, s)| contender_timing(c.name(), s))
        .collect();
    t.headline = headline(&t.contenders);
    t.noisy = t
        .contenders
        .iter()
        .any(|c| c.e2e_us.as_ref().is_some_and(|s| s.noisy));
    // `reps_done` counts SUCCESSFUL samples (the fewest any contender produced), not attempts
    t.min_samples = t
        .contenders
        .iter()
        .map(|c| c.e2e_us.as_ref().map_or(0, |s| s.n))
        .min()
        .unwrap_or(0);
    t.reps_done = t.min_samples;
    for c in &t.contenders {
        let ok = c.e2e_us.as_ref().map_or(0, |s| s.n);
        if c.failed_reps > 0 {
            t.notes.push(format!(
                "{}: {} of {} timed repetition(s) failed and were excluded ({ok} successful sample(s))",
                c.name, c.failed_reps, t.reps_attempted
            ));
        }
        if ok < MIN_TIMING_SAMPLES {
            t.notes.push(format!(
                "{} has only {ok} successful end-to-end sample(s) (at least {MIN_TIMING_SAMPLES} are needed): the timing is `unstable`",
                c.name
            ));
        }
    }
    let unmeasured_ng: Vec<&str> = t
        .contenders
        .iter()
        .filter(|c| c.name.starts_with("ngspice_") && c.e2e_us.as_ref().map_or(0, |s| s.n) == 0)
        .map(|c| c.name.as_str())
        .collect();
    if !unmeasured_ng.is_empty()
        && t.contenders
            .iter()
            .any(|c| c.name.starts_with("ngspice_") && c.e2e_us.is_some())
    {
        t.notes.push(format!(
            "the headline speedup is against the remaining ngspice solver only: {} has no successful repetition",
            unmeasured_ng.join(", ")
        ));
    }
    if t.reps_done < 5 {
        t.notes.push(format!(
            "only {} successful repetition(s): min/median are weak estimates",
            t.reps_done
        ));
    }
    t
}

fn measure_floor(ctx: &Ctx) -> Floors {
    let reps = 15usize;
    let dir = ctx.tmp.path().join("floor");
    let _ = std::fs::create_dir_all(&dir);
    let deck = dir.join("floor.sp");
    if write_file(&deck, "startup floor\nV1 a 0 1\nR1 a 0 1k\n.op\n.end\n").is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        return Floors {
            status: "scratch_write_failed".to_string(),
            reps,
            ngspice_us: None,
            rustspice_us: None,
        };
    }
    let contenders = [Contender::Ng(NgSolver::Sparse, deck.clone()), Contender::Rs];
    let mut samples = [Samples::default(), Samples::default()];
    for rep in 0..reps + 1 {
        let order: Vec<usize> = if rep % 2 == 0 { vec![0, 1] } else { vec![1, 0] };
        for i in order {
            let mut s = Samples::default();
            timed_run(ctx, &contenders[i], &dir, &deck, &dir, &mut s, false);
            if rep > 0 {
                samples[i].e2e.extend(s.e2e);
                samples[i].failed += s.failed;
            } // repetition 0 is the warm-up
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let ok = samples[0].failed == 0
        && samples[1].failed == 0
        && !samples[0].e2e.is_empty()
        && !samples[1].e2e.is_empty();
    Floors {
        status: if ok { "ok" } else { "partial_or_failed" }.to_string(),
        reps,
        ngspice_us: summarize(&samples[0].e2e),
        rustspice_us: summarize(&samples[1].e2e),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Aggregation: series, fuzz, scaling
// ---------------------------------------------------------------------------------------------------------

fn best_ng(t: &TimingResult, f: impl Fn(&ContenderTiming) -> Option<f64>) -> Option<f64> {
    t.contenders
        .iter()
        .filter(|c| c.name.starts_with("ngspice_"))
        .filter_map(f)
        .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.min(v))))
}

fn rs_of(t: &TimingResult, f: impl Fn(&ContenderTiming) -> Option<f64>) -> Option<f64> {
    t.contenders
        .iter()
        .find(|c| c.name == "rustspice")
        .and_then(f)
}

/// A known gap of a work-precision series (`fixed-step-accuracy`) is a property of the coarse members: a fine
/// member that is within tolerance is a `pass`, not a closed gap, as long as another member of the same series
/// still confirms it. When no member confirms it the gap really is closed and stays `gap_closed`.
fn reconcile_series_gaps(results: &mut [CaseResult]) {
    let confirmed: HashSet<String> = results
        .iter()
        .filter(|c| c.status == Status::KnownGapConfirmed)
        .filter_map(|c| c.series.as_ref().map(|s| s.base.clone()))
        .collect();
    for c in results.iter_mut() {
        if c.status != Status::GapClosed {
            continue;
        }
        let Some(base) = c.series.as_ref().map(|s| s.base.clone()) else {
            continue;
        };
        if confirmed.contains(&base) {
            c.status = Status::Pass;
            c.notes.push(format!(
                "series member within tolerance at this step; the known gap `{}` still holds at other members of the series",
                c.known_gap.clone().unwrap_or_default()
            ));
        }
    }
}

fn build_series(cases: &[CaseResult]) -> Vec<SeriesResult> {
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for c in cases {
        if let Some(s) = &c.series {
            by.entry(s.base.clone()).or_default().push(c);
        }
    }
    let mut out = Vec::new();
    for (base, mut cs) in by {
        cs.sort_by(|a, b| {
            let ta = a.series.as_ref().and_then(|s| s.tstep).unwrap_or(0.0);
            let tb = b.series.as_ref().and_then(|s| s.tstep).unwrap_or(0.0);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut pts = Vec::new();
        for c in &cs {
            let t = c.timing.as_ref();
            pts.push(SeriesPoint {
                id: c.id.clone(),
                tstep: c.series.as_ref().and_then(|s| s.tstep),
                status: c.status,
                err_rs: c.accuracy.as_ref().and_then(|a| a.err_rs),
                err_ng: c.accuracy.as_ref().and_then(|a| a.err_ng),
                rs_solve_us: t.and_then(|t| rs_of(t, |c| c.analysis_us.as_ref().map(|s| s.median))),
                ng_solve_us: t
                    .and_then(|t| best_ng(t, |c| c.analysis_us.as_ref().map(|s| s.median))),
                rs_e2e_us: t.and_then(|t| rs_of(t, |c| c.e2e_us.as_ref().map(|s| s.median))),
                ng_e2e_us: t.and_then(|t| best_ng(t, |c| c.e2e_us.as_ref().map(|s| s.median))),
            });
        }
        let curve = |e: fn(&SeriesPoint) -> Option<f64>,
                     t: fn(&SeriesPoint) -> Option<f64>|
         -> Vec<(f64, f64)> {
            pts.iter().filter_map(|p| Some((e(p)?, t(p)?))).collect()
        };
        let eq_solve = time_at_equal_error(
            &curve(|p| p.err_rs, |p| p.rs_solve_us),
            &curve(|p| p.err_ng, |p| p.ng_solve_us),
            5,
        );
        let eq_e2e = time_at_equal_error(
            &curve(|p| p.err_rs, |p| p.rs_e2e_us),
            &curve(|p| p.err_ng, |p| p.ng_e2e_us),
            5,
        );
        out.push(SeriesResult {
            title: cs.first().map(|c| c.title.clone()).unwrap_or_default(),
            base,
            points: pts,
            equal_error_solve: eq_solve,
            equal_error_e2e: eq_e2e,
        });
    }
    out
}

fn build_fuzz(cases: &[CaseResult]) -> Vec<FuzzSummary> {
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for c in cases {
        if c.origin == "fuzz" || c.origin == "mutant" {
            by.entry(c.family.clone()).or_default().push(c);
        }
    }
    by.into_iter()
        .map(|(family, cs)| {
            let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
            for c in &cs {
                *by_status.entry(c.status.as_str().to_string()).or_default() += 1;
            }
            // failing cases grouped by signature (the key of the artifact cap), in case order
            let mut sigs: Vec<SignatureCount> = Vec::new();
            let mut first_failures: Vec<FuzzFailure> = Vec::new();
            for c in cs.iter().filter(|c| c.status.is_fail()) {
                let sig = c
                    .signature
                    .clone()
                    .unwrap_or_else(|| c.status.as_str().to_string());
                let kept = c.artifacts.is_some();
                match sigs.iter_mut().find(|x| x.signature == sig) {
                    Some(x) => {
                        x.count += 1;
                        x.artifacts_kept += usize::from(kept);
                    }
                    None => sigs.push(SignatureCount {
                        signature: sig.clone(),
                        count: 1,
                        artifacts_kept: usize::from(kept),
                        first_id: c.id.clone(),
                    }),
                }
                if kept && first_failures.len() < 12 {
                    first_failures.push(FuzzFailure {
                        id: c.id.clone(),
                        status: c.status,
                        note: c
                            .failures
                            .first()
                            .or(c.notes.first())
                            .cloned()
                            .unwrap_or_default(),
                        artifact: c.artifacts.clone(),
                    });
                }
            }
            FuzzSummary {
                family,
                count: cs.len(),
                by_status,
                first_failures,
                signatures: sigs,
            }
        })
        .collect()
}

fn build_scaling(cases: &[CaseResult]) -> Vec<ScalingResult> {
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for c in cases {
        // Only a family is fitted: generated families and static decks with an explicit `family:<id>` tag. Static
        // decks without one all carry the family `static`, an arbitrary mix of analyses and circuits, and a fit over
        // them is meaningless (34 points gave an exponent of 0.36 with R2 = 0.05).
        let explicit_family = !(c.origin == "static" && c.family == "static");
        if c.series.is_none() && c.timing.is_some() && c.size.is_some() && explicit_family {
            by.entry(c.family.clone()).or_default().push(c);
        }
    }
    let mut out = Vec::new();
    for (family, mut cs) in by {
        cs.sort_by_key(|c| c.size);
        let named = |t: &TimingResult, n: &str, solve: bool| -> Option<f64> {
            t.contenders
                .iter()
                .find(|c| c.name == n)
                .and_then(|c| {
                    if solve {
                        c.analysis_us.as_ref()
                    } else {
                        c.e2e_us.as_ref()
                    }
                })
                .map(|s| s.median)
        };
        let child_peak = |c: &CaseResult, n: &str| -> Option<u64> {
            c.rss
                .as_ref()
                .and_then(|r| r.children.iter().find(|x| x.child == n))
                .and_then(|x| x.peak_kib)
        };
        let points: Vec<ScalingPoint> = cs
            .iter()
            .map(|c| {
                let t = c.timing.as_ref().unwrap();
                let size = c.size.unwrap();
                let rs_rss = child_peak(c, "rustspice");
                let ng_sparse_rss = child_peak(c, "ngspice_sparse");
                let ng_klu_rss = child_peak(c, "ngspice_klu");
                let ng_peak = match (ng_sparse_rss, ng_klu_rss) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                let per_unknown = |k: Option<u64>| -> Option<f64> {
                    k.filter(|_| size > 0).map(|k| k as f64 / size as f64)
                };
                ScalingPoint {
                    id: c.id.clone(),
                    size,
                    ng_sparse_solve_us: named(t, "ngspice_sparse", true),
                    ng_klu_solve_us: named(t, "ngspice_klu", true),
                    rs_solve_us: named(t, "rustspice", true),
                    ng_sparse_e2e_us: named(t, "ngspice_sparse", false),
                    ng_klu_e2e_us: named(t, "ngspice_klu", false),
                    rs_e2e_us: named(t, "rustspice", false),
                    ng_sparse_rss_kib: ng_sparse_rss,
                    ng_klu_rss_kib: ng_klu_rss,
                    rs_rss_kib: rs_rss,
                    rs_kib_per_unknown: per_unknown(rs_rss),
                    ng_kib_per_unknown: per_unknown(ng_peak),
                }
            })
            .collect();
        if points.len() < 2 {
            continue;
        }
        let fit = |f: fn(&ScalingPoint) -> Option<f64>| {
            loglog_fit(
                &points
                    .iter()
                    .filter_map(|p| Some((p.size as f64, f(p)?)))
                    .collect::<Vec<_>>(),
            )
        };
        out.push(ScalingResult {
            family,
            fit: ScalingFits {
                ng_sparse: fit(|p| p.ng_sparse_solve_us),
                ng_klu: fit(|p| p.ng_klu_solve_us),
                rs: fit(|p| p.rs_solve_us),
            },
            fit_e2e: ScalingFits {
                ng_sparse: fit(|p| p.ng_sparse_e2e_us),
                ng_klu: fit(|p| p.ng_klu_e2e_us),
                rs: fit(|p| p.rs_e2e_us),
            },
            fit_rss: ScalingFits {
                ng_sparse: fit(|p| p.ng_sparse_rss_kib.map(|k| k as f64)),
                ng_klu: fit(|p| p.ng_klu_rss_kib.map(|k| k as f64)),
                rs: fit(|p| p.rs_rss_kib.map(|k| k as f64)),
            },
            points,
        });
    }
    out
}

/// Timed cases on which RustSpice is slower than the faster ngspice solver END TO END: a measured (timing status
/// `measured`), non-noisy median speedup below 1.0 and no performance known-gap slug (those are judged by their own
/// status). Slowest first. Cases that look slower but have a noisy timing are returned separately by id.
pub fn build_slower(cases: &[CaseResult]) -> (Vec<SlowerCase>, Vec<String>) {
    let mut slower: Vec<SlowerCase> = Vec::new();
    let mut noisy: Vec<String> = Vec::new();
    for c in cases {
        let Some(t) = c.timing.as_ref() else { continue };
        if c.timing_status != "measured" {
            continue;
        }
        if c.known_gap
            .as_deref()
            .is_some_and(|g| PERF_GAP_SLUGS.contains(&g))
        {
            continue;
        }
        let Some(x) = t.headline.e2e_median.filter(|x| x.is_finite() && *x < 1.0) else {
            continue;
        };
        if t.noisy {
            noisy.push(c.id.clone());
            continue;
        }
        let ng_med = |f: fn(&ContenderTiming) -> Option<f64>| best_ng(t, f);
        slower.push(SlowerCase {
            id: c.id.clone(),
            size: c.size,
            speedup_e2e: x,
            vs: t.headline.e2e_vs.clone().unwrap_or_default(),
            solve_speedup: t.headline.solve_median,
            rs_solver_kind: c
                .rustspice
                .as_ref()
                .and_then(|r| r.stats.as_ref())
                .and_then(|s| s.solver_kind.clone()),
            rs_e2e_us: rs_of(t, |x| x.e2e_us.as_ref().map(|s| s.median)),
            ng_e2e_us: ng_med(|x| x.e2e_us.as_ref().map(|s| s.median)),
            rs_compile_us: rs_of(t, |x| x.compile_us.as_ref().map(|s| s.median)),
            rs_parse_us: rs_of(t, |x| x.parse_us.as_ref().map(|s| s.median)),
            rs_solve_us: rs_of(t, |x| x.analysis_us.as_ref().map(|s| s.median)),
            ng_solve_us: ng_med(|x| x.analysis_us.as_ref().map(|s| s.median)),
            known_gap: c.known_gap.clone(),
        });
    }
    slower.sort_by(|a, b| {
        a.speedup_e2e
            .partial_cmp(&b.speedup_e2e)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    (slower, noisy)
}

// ---------------------------------------------------------------------------------------------------------
// Host information and the driver
// ---------------------------------------------------------------------------------------------------------

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown cpu".to_string())
}

fn git_info(dir: &Path) -> (Option<String>, Option<bool>) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let head = run(&["rev-parse", "--short", "HEAD"]);
    let dirty = run(&["status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty());
    (head, dirty)
}

fn list_cases(cases: &[Case]) {
    for c in cases {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\ttiming={}\test_rss={}\t{}",
            c.id,
            c.meta.group.as_str(),
            c.family,
            c.origin.as_str(),
            c.meta.expect.as_str(),
            c.meta.known_gap.as_deref().unwrap_or("-"),
            if c.meta.timing { "yes" } else { "no" },
            c.est_rss_mib.map_or("-".to_string(), |x| x.to_string()),
            c.meta.title
        );
    }
}

fn write_out(path: &Path, text: &str) -> bool {
    if let Some(p) = path.parent() {
        if !p.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(p);
        }
    }
    match std::fs::write(path, text) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("error: cannot write {}: {e}", path.display());
            false
        }
    }
}

/// Smallest `--max-rss-mib`: a smaller cap kills every child at once and reports a usage error as a RustSpice panic.
pub const MIN_RSS_CAP_MIB: u64 = 16;
/// Largest accepted `--fuzz-count` (decks per family; the mutants are twice this).
pub const MAX_FUZZ_COUNT: usize = 5000;
/// Largest accepted `--reps`.
pub const MAX_REPS: usize = 1000;
/// Largest accepted `--case-timeout` (seconds).
pub const MAX_CASE_TIMEOUT_S: u64 = 86_400;
/// Free space the scratch directory needs at start-up (bytes).
const MIN_SCRATCH_FREE_BYTES: u64 = 64 << 20;

/// Numeric arguments that would otherwise be accepted and then misreported (`--max-rss-mib 0` ended every child at
/// once and the suite called it "unbounded memory growth of RustSpice"). `Err` is a usage error, exit code 2.
pub fn validate_numeric_args(args: &SuiteArgs) -> Result<(), String> {
    if args.max_rss_mib < MIN_RSS_CAP_MIB {
        return Err(format!(
            "--max-rss-mib {} is too small (at least {MIN_RSS_CAP_MIB} MiB: both tools need more than that to start)",
            args.max_rss_mib
        ));
    }
    if args.case_timeout == 0 || args.case_timeout > MAX_CASE_TIMEOUT_S {
        return Err(format!(
            "--case-timeout {} is out of range (1..={MAX_CASE_TIMEOUT_S} seconds)",
            args.case_timeout
        ));
    }
    if args.reps == 0 || args.reps > MAX_REPS {
        return Err(format!(
            "--reps {} is out of range (1..={MAX_REPS})",
            args.reps
        ));
    }
    if let Some(n) = args.fuzz_count {
        if n > MAX_FUZZ_COUNT {
            return Err(format!(
                "--fuzz-count {n} is too large (at most {MAX_FUZZ_COUNT} decks per family)"
            ));
        }
    }
    if let Some(cpu) = args.pin_cpu {
        rustsim::validate_pin_cpu(cpu).map_err(|e| format!("--pin-cpu {cpu}: {e}"))?;
    }
    Ok(())
}

/// Solvers that actually ran (from the results): `sparse`, `klu`.
fn solvers_run(results: &[CaseResult]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    for c in results {
        if let Some(ng) = &c.ngspice {
            if !matches!(ng.status.as_str(), "skipped_unbounded" | "not_run") {
                seen.insert(ng.solver.clone());
            }
        }
        if let Some(t) = &c.timing {
            for ct in &t.contenders {
                if let Some(n) = ct.name.strip_prefix("ngspice_") {
                    seen.insert(n.to_string());
                }
            }
        }
    }
    ["sparse", "klu"]
        .iter()
        .filter(|s| seen.contains(**s))
        .map(|s| s.to_string())
        .collect()
}

/// Start-up check that the ngspice program can simulate at all (`--version` succeeding says nothing: a wrapper that
/// answers the probe and then fails every run made every comparable case `ngspice_error`, which never fails the run).
/// A resistor divider is run and its answer is checked against the value known without any simulator
/// (5 V * 3k / 4k = 3.75 V). `Err` is the reason ngspice is unusable.
fn ngspice_selftest(ngspice: &str, scratch: &Path) -> Result<(), String> {
    let dir = scratch.join("ngspice-selftest");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let deck = dir.join("selftest.sp");
    write_file(
        &deck,
        "ngspice start-up check\nV1 in 0 DC 5\nR1 in out 1k\nR2 out 0 3k\n.op\n.end\n",
    )?;
    let lim = Limits {
        timeout: Duration::from_secs(30),
        max_rss_kib: Some(1 << 20),
        pin_cpu: None,
    };
    // a start-up check is not a timed run: its child may die with the harness
    let _orphans = rustsim::orphan_guard(true);
    let run = ngspice::run(ngspice, &deck, &dir.join("selftest.raw"), &lim, None, true);
    let outcome = if run.class != NgClass::Ok {
        Err(format!(
            "the start-up check (a resistor divider) did not simulate: {}",
            ng_class_msg(&run.class)
        ))
    } else {
        match &run.plots {
            Ok(plots) => {
                let v = plots.iter().find_map(|p| {
                    let k = p.vars.iter().position(|v| v.name == "v(out)")?;
                    p.cols.get(k)?.first().copied()
                });
                match v {
                    Some(x) if (x - 3.75).abs() <= 1e-6 => Ok(()),
                    Some(x) => Err(format!(
                        "the start-up check gave v(out) = {x} V for a 5 V / 1k / 3k divider (3.75 V expected)"
                    )),
                    None => Err("the start-up check wrote no v(out) vector".to_string()),
                }
            }
            Err(e) => Err(format!(
                "the start-up check wrote an unreadable rawfile: {e}"
            )),
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

/// Entry point of `rustspice bench-suite`. Returns the process exit code (0 ok, 1 failures, 2 usage/environment).
pub fn run_suite(mut args: SuiteArgs) -> i32 {
    // Children run in the deck's directory (relative includes) and may crash (ngspice segfaults on some hostile
    // decks): never let a core dump land in the repository, and keep a relative executable path valid.
    let no_core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    unsafe {
        libc::setrlimit(libc::RLIMIT_CORE, &no_core);
        // an ignored SIGCHLD (inherited through `exec`) makes `wait4` fail with ECHILD for every child
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
    }
    if args.ngspice.contains('/') {
        if let Ok(p) = Path::new(&args.ngspice).canonicalize() {
            args.ngspice = p.display().to_string();
        }
    }
    if let Err(e) = validate_numeric_args(&args) {
        eprintln!("error: {e}");
        return 2;
    }
    let Some(scale) = Scale::parse(&args.scale) else {
        eprintln!("error: --scale must be small|medium|large");
        return 2;
    };
    for g in &args.group {
        if Group::parse(g).is_none() {
            eprintln!("error: unknown group `{g}` (capability|accuracy|robustness|performance)");
            return 2;
        }
    }
    let fuzz_count = args.fuzz_count.unwrap_or(match scale {
        Scale::Small => 25,
        Scale::Medium => 100,
        Scale::Large => 400,
    });
    let cases = match load_cases(&args, scale, fuzz_count) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if cases.is_empty() {
        eprintln!(
            "error: no benchmark cases found (cases dir `{}`, groups {:?}, filter {:?})",
            args.cases.display(),
            args.group,
            args.filter
        );
        return 2;
    }
    if args.list {
        list_cases(&cases);
        return 0;
    }
    if scale == Scale::Large {
        let max = cases
            .iter()
            .filter_map(|c| c.est_rss_mib)
            .max()
            .unwrap_or(0);
        eprintln!(
            "scale large: {} cases, largest RustSpice RSS estimate {} MiB (cases run sequentially); cases above --max-rss-mib {} are skipped",
            cases.len(),
            max,
            args.max_rss_mib
        );
    }
    // SUITE_SPEC section 3: at least five interleaved repetitions
    let reps = args.reps.max(5);
    if args.reps < 5 {
        eprintln!("warning: --reps {} raised to the minimum of 5", args.reps);
    }
    // from here on children and a RAM-backed scratch directory exist: SIGTERM / SIGINT / SIGHUP kill the children
    // and remove the scratch directory before the process exits with 128 + signal
    rustsim::install_termination_cleanup();
    // the in-process oracles get a wall-clock budget of min(10 s, case_timeout / 2) (they run outside the case guard)
    oracle::set_case_timeout(Duration::from_secs(args.case_timeout));
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: cannot locate the rustspice executable: {e}");
            return 2;
        }
    };
    let tmp = match TmpDir::new("suite") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot create a temporary directory: {e}");
            return 2;
        }
    };
    if let Some(free) = tmp.free_bytes() {
        if free < MIN_SCRATCH_FREE_BYTES {
            eprintln!(
                "error: only {} MiB are free in the scratch directory {} (at least {} MiB are needed)",
                free >> 20,
                tmp.path().display(),
                MIN_SCRATCH_FREE_BYTES >> 20
            );
            return 2;
        }
    }
    // a scratch directory that cannot be written would hand both tools an empty deck for every case: stop now
    if let Err(e) = write_file(&tmp.path().join("write-probe"), b"rustspice bench-suite\n") {
        eprintln!("error: {e}; the scratch file system is full or read-only");
        return 2;
    }
    let (ng_version, mut ng_reason) = match ngspice::version_probe(&args.ngspice) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    if ng_version.is_some() {
        if let Err(e) = ngspice_selftest(&args.ngspice, tmp.path()) {
            ng_reason = Some(e);
        }
    }
    let ng_installed = ng_version.is_some() && ng_reason.is_none();
    if let Some(why) = &ng_reason {
        eprintln!("warning: ngspice is not usable: {why}; every case is `unverified`");
    }
    let artifacts = args
        .artifacts
        .clone()
        .unwrap_or_else(|| PathBuf::from("bench-artifacts"));
    let lim = Limits {
        timeout: Duration::from_secs(args.case_timeout.max(1)),
        max_rss_kib: Some(args.max_rss_mib.saturating_mul(1024)),
        pin_cpu: args.pin_cpu,
    };
    let mut ctx = Ctx {
        lim: lim.clone(),
        ng_lim: lim,
        args: args.clone(),
        exe,
        tmp,
        ng_installed,
        ng_reason: ng_reason.clone(),
        ref_cache: None,
        artifacts,
        retained: HashMap::new(),
        artifact_budget: MAX_ARTIFACT_BYTES,
        reps,
    };
    let loadavg_start = rustsim::loadavg();
    let any_timing = !args.no_timing && cases.iter().any(|c| c.meta.timing);
    let startup_floor = if ng_installed && any_timing {
        Some(measure_floor(&ctx))
    } else {
        None
    };

    let n = cases.len();
    let mut results: Vec<CaseResult> = Vec::with_capacity(n);
    for (i, case) in cases.iter().enumerate() {
        let r = run_case(&mut ctx, case);
        eprintln!(
            "[{:>4}/{}] {:<44} {} ({:.0} ms)",
            i + 1,
            n,
            case.id,
            r.status.as_str(),
            r.case_wall_ms
        );
        results.push(r);
        if let Some(sig) = rustsim::terminating_signal() {
            // the cleanup thread ends the process (children killed, scratch removed); give it the time
            std::thread::sleep(Duration::from_secs(10));
            return 128 + sig;
        }
    }
    let loadavg_end = rustsim::loadavg();
    reconcile_series_gaps(&mut results);
    let series = build_series(&results);
    let fuzz = build_fuzz(&results);
    let scaling = build_scaling(&results);
    let (slower, slower_noisy) = build_slower(&results);
    let summary = summarize_counts(&results, ng_installed);
    let teeth_summary = if args.teeth {
        let mut t = TeethSummary {
            factor: teeth::FACTOR,
            ..Default::default()
        };
        for c in &results {
            if let Some(x) = &c.teeth {
                t.add(&c.id, &c.group, x);
            }
        }
        Some(t)
    } else {
        None
    };
    let solvers_tested = solvers_run(&results);
    // the version of the BINARY under test: the repository of the directory this binary was compiled in (not the
    // corpus, which can live in any other repository), and the corpus repository on its own
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (git, dirty, source_dir) = if manifest_dir.is_dir() {
        let (g, d) = git_info(manifest_dir);
        (g, d, Some(manifest_dir.display().to_string()))
    } else {
        (None, None, None)
    };
    let corpus = if args.families_only || !args.cases.is_dir() {
        None
    } else {
        let (g, d) = git_info(&args.cases);
        Some(CorpusInfo {
            path: args.cases.display().to_string(),
            git: g,
            dirty: d,
        })
    };
    let pin_failed = rustsim::pin_failures();
    let (pin_cpu, pin_status) = match (args.pin_cpu, pin_failed) {
        (None, _) => (None, "not_requested".to_string()),
        (Some(_), 0) => (args.pin_cpu, "applied".to_string()),
        (Some(c), k) => (
            None,
            format!("failed: {k} child start(s) could not be pinned to CPU {c}"),
        ),
    };
    let d = meta::Meta::defaults("x");
    let rep = SuiteReport {
        schema: 1,
        generated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        host: HostInfo {
            cpu: cpu_model(),
            threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            loadavg_start,
            loadavg_end,
            pin_cpu,
            tmp_on_tmpfs: ctx.tmp.on_tmpfs(),
            pin_status,
        },
        ngspice: NgInfo {
            version: ng_version,
            path: args.ngspice.clone(),
            installed: ng_installed,
            solvers_tested,
            unusable_reason: ng_reason,
        },
        rustspice: RsInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git,
            dirty,
            build_profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
            .to_string(),
            source_dir,
        },
        tolerances: Tolerances {
            tol_rel: d.tol_rel,
            tol_abs: d.tol_abs,
            tol_norm: d.tol_norm,
            floor: d.floor,
            tight_reference: TIGHT_OPTIONS.to_string(),
            tran_reference: TRAN_REF_OPTIONS.to_string(),
            noisy_threshold: NOISY_THRESHOLD,
        },
        args: std::env::args().skip(1).collect(),
        scale: args.scale.clone(),
        seed: args.seed,
        startup_floor,
        cases: results,
        series,
        fuzz,
        scaling,
        slower_than_ngspice: slower,
        slower_than_ngspice_noisy: slower_noisy,
        teeth: teeth_summary,
        corpus,
        summary,
    };
    let mut io_ok = true;
    if let Some(p) = &args.json {
        io_ok &= write_out(p, &to_json(&rep));
    }
    if let Some(p) = &args.report {
        io_ok &= write_out(p, &to_markdown(&rep));
    }
    eprintln!(
        "verdict: {} ({} cases; {})",
        rep.summary.verdict_line,
        rep.summary.total,
        Status::ALL
            .iter()
            .filter_map(|s| rep
                .summary
                .counts
                .get(s.as_str())
                .filter(|n| **n > 0)
                .map(|n| format!("{}={}", s.as_str(), n)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for u in rep.summary.unverified.iter().take(10) {
        eprintln!("  unverified: {} - {}", u.id, u.reason);
    }
    if rep.summary.unverified.len() > 10 {
        eprintln!(
            "  ... {} more unverified case(s), see the report",
            rep.summary.unverified.len() - 10
        );
    }
    if !ng_installed && !args.allow_unverified {
        eprintln!(
            "error: ngspice is missing or not usable; pass --allow-unverified to accept an all-unverified run"
        );
        return 2;
    }
    if !io_ok {
        return 2;
    }
    let infra: Vec<&CaseResult> = rep
        .cases
        .iter()
        .filter(|c| c.harness_failure.is_some())
        .collect();
    if let Some(first) = infra.first() {
        eprintln!(
            "error: {} case(s) could not be run because the harness environment failed (first: {} - {}); the run is not trustworthy",
            infra.len(),
            first.id,
            first.harness_failure.as_deref().unwrap_or("")
        );
        return 2;
    }
    if rep.summary.verdict == "FAIL" {
        return 1;
    }
    if args.strict_unverified && rep.summary.unverified_count > 0 {
        eprintln!(
            "error: --strict-unverified: {} case(s) could not be judged",
            rep.summary.unverified_count
        );
        return 2;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ng_deck_rewriting_order() {
        let d = "t\nR1 a 0 1\n.tran 1n 1u\n.end\n";
        let out = ng_deck_text(d, NgSolver::Klu, Some("reltol=1e-9"), Some(5e-11)).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "t");
        assert!(lines.contains(&".options klu"));
        assert!(lines.contains(&".options reltol=1e-9"));
        assert!(out.contains(".tran 1n 1u 0 5.000000000e-11"));
        assert!(ng_deck_text("t\n.op\n", NgSolver::Sparse, None, Some(1e-9)).is_err());
        assert_eq!(ng_deck_text(d, NgSolver::Sparse, None, None).unwrap(), d);
    }

    #[test]
    fn headline_uses_fastest_ngspice_solver() {
        let mk = |name: &str, e2e: f64, sol: f64| ContenderTiming {
            name: name.to_string(),
            e2e_us: summarize(&[e2e, e2e, e2e]),
            analysis_us: summarize(&[sol, sol, sol]),
            ..Default::default()
        };
        let h = headline(&[
            mk("ngspice_sparse", 8000.0, 400.0),
            mk("ngspice_klu", 6000.0, 900.0),
            mk("rustspice", 2000.0, 100.0),
        ]);
        assert_eq!(h.e2e_median, Some(3.0));
        assert_eq!(h.e2e_vs.as_deref(), Some("ngspice_klu"));
        assert_eq!(h.solve_median, Some(4.0));
        assert_eq!(h.solve_vs.as_deref(), Some("ngspice_sparse"));
        assert_eq!(h.per_solver.len(), 2);
        // missing rustspice data -> no speedup, not zero
        let h = headline(&[mk("ngspice_sparse", 8000.0, 400.0)]);
        assert!(h.e2e_median.is_none() && h.solve_median.is_none());
    }

    #[test]
    fn budget_errors_are_recognised() {
        let e = rawfile::RawError::Budget("plot is 2 GiB".to_string());
        assert!(is_budget(&e.to_string()));
        assert!(is_budget(&format!("error: rawfile unreadable: {e}")));
        assert!(!is_budget("rawfile truncated"));
    }

    #[test]
    fn series_cases_expand_and_ids_sorted() {
        let deck = "t\n* bench.id: s1\n* bench.group: accuracy\n* bench.series.tstep: 10n 5n\nV1 a 0 1\nR1 a 0 1\n.tran 10n 1u\n.end\n";
        let mut out = Vec::new();
        make_cases(deck, Some("s1"), None, Origin::Static, None, None, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "s1__s1");
        assert_eq!(out[1].tstep, Some(5e-9));
        assert!(out[1].meta.timing);
        // invalid metadata yields one case carrying the diagnostics
        let mut out = Vec::new();
        make_cases(
            "t\n* bench.id: z\n* bench.expect: maybe\n.op\n",
            Some("z"),
            None,
            Origin::Static,
            None,
            None,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert!(!out[0].diags.is_empty());
    }

    fn test_args(cases: &Path) -> SuiteArgs {
        SuiteArgs {
            cases: cases.to_path_buf(),
            group: Vec::new(),
            filter: None,
            family: Vec::new(),
            scale: "small".to_string(),
            reps: 5,
            case_timeout: 20,
            max_rss_mib: 2048,
            seed: 1,
            fuzz_count: None,
            json: None,
            report: None,
            artifacts: None,
            ngspice: "ngspice".to_string(),
            pin_cpu: None,
            no_timing: true,
            no_fuzz: true,
            teeth: false,
            list: false,
            allow_unverified: false,
            strict_unverified: false,
            families_only: false,
        }
    }

    fn one_case(deck: &str, stem: &str) -> Case {
        let mut out = Vec::new();
        make_cases(deck, Some(stem), None, Origin::Static, None, None, &mut out);
        out.remove(0)
    }

    #[test]
    fn static_decks_keep_their_bytes_sidecars_and_directory() {
        let t = TmpDir::new("load-cases").unwrap();
        let sub = t.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        // corpus-directory prefix on the stem, plain deck
        std::fs::write(
            sub.join("rob_hs_x.sp"),
            "title\n* bench.id: x\n* bench.group: robustness\n.op\n.end\n",
        )
        .unwrap();
        // an empty deck takes its metadata from the sidecar
        std::fs::write(sub.join("empty.sp"), "").unwrap();
        std::fs::write(
            sub.join("empty.sp.meta"),
            "bench.id: empty\nbench.group: robustness\nbench.expect: loud_error\n",
        )
        .unwrap();
        // invalid UTF-8 in the title: metadata is read lossily, the bytes stay exact
        let latin1: &[u8] =
            b"t\xe9\n* bench.id: latin1\n* bench.group: robustness\nV1 a 0 1\n.op\n.end\n";
        std::fs::write(sub.join("latin1.sp"), latin1).unwrap();
        let cases = load_cases(&test_args(t.path()), Scale::Small, 0).unwrap();
        let get = |id: &str| {
            cases
                .iter()
                .find(|c| c.id == id)
                .unwrap_or_else(|| panic!("{id}"))
        };

        let x = get("rob_hs_x");
        assert!(x.diags.is_empty(), "{:?}", x.diags);
        assert!(x
            .notes
            .iter()
            .any(|n| n.contains("lacks the file-stem prefix")));
        assert_eq!(
            x.src_dir.as_deref(),
            Some(sub.canonicalize().unwrap().as_path())
        );

        let e = get("empty");
        assert!(e.diags.is_empty(), "{:?}", e.diags);
        assert_eq!(e.meta.expect, Expect::LoudError);
        assert!(e.deck.is_empty() && e.raw.is_none());
        assert!(e.notes.iter().any(|n| n.contains("sidecar")));

        let l = get("latin1");
        assert!(l.diags.is_empty(), "{:?}", l.diags);
        assert_eq!(l.raw.as_deref(), Some(latin1));
        let ng = ng_deck_bytes(l, NgSolver::Klu, None, None).unwrap();
        assert_eq!(&ng[..3], b"t\xe9\n");
        assert!(ng.starts_with(b"t\xe9\n.options klu\n* bench.id: latin1\n"));
        assert!(ng_deck_bytes(l, NgSolver::Sparse, None, Some(1e-9)).is_err());
        // a plain deck goes through the text path unchanged
        assert_eq!(
            ng_deck_bytes(x, NgSolver::Sparse, None, None).unwrap(),
            x.deck.clone().into_bytes()
        );
    }

    #[test]
    fn literal_reference_options_default_to_tstep_over_20() {
        let tran = "t\n* bench.id: r\n* bench.group: accuracy\n* bench.ref: reltol=1e-9 abstol=1e-15\nV1 a 0 1\nR1 a 0 1\n.tran 10n 1u\n.end\n";
        let c = one_case(tran, "r");
        let s = ref_spec(&c, true);
        assert_eq!(s.options.as_deref(), Some("reltol=1e-9 abstol=1e-15"));
        assert_eq!(s.tmax, Some(10e-9 / 20.0));
        // an explicit tmax wins, `tight` keeps no automatic tmax, op decks never get one
        let c = one_case(&tran.replace("V1 a", "* bench.ref.tmax: 1n\nV1 a"), "r");
        assert_eq!(ref_spec(&c, true).tmax, Some(1e-9));
        let c = one_case(&tran.replace("reltol=1e-9 abstol=1e-15", "tight"), "r");
        assert_eq!(ref_spec(&c, true).tmax, None);
        let op = "t\n* bench.id: r\n* bench.group: accuracy\n* bench.ref: reltol=1e-9\nV1 a 0 1\nR1 a 0 1\n.op\n.end\n";
        assert_eq!(ref_spec(&one_case(op, "r"), false).tmax, None);
        // default mode is unchanged
        let d = tran.replace("* bench.ref: reltol=1e-9 abstol=1e-15\n", "");
        let s = ref_spec(&one_case(&d, "r"), true);
        assert_eq!((s.label, s.tmax), ("converged_tran", Some(10e-9 / 20.0)));
        // a parameterised stop time still gets the tmax from the numeric time step
        let p = tran.replace(".tran 10n 1u", ".tran 10n {1u*n}");
        assert_eq!(ref_spec(&one_case(&p, "r"), true).tmax, Some(10e-9 / 20.0));
    }

    #[test]
    fn coarse_series_members_are_measurements_not_failures() {
        let deck = "t\n* bench.id: s\n* bench.group: accuracy\n* bench.series.tstep: 20n 10n 5n\nV1 a 0 1\nR1 a 0 1\n.tran 10n 1u\n.end\n";
        let mut out = Vec::new();
        make_cases(deck, Some("s"), None, Origin::Static, None, None, &mut out);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|c| c.nominal_tstep == Some(10e-9)));
        let verdict = || compare::Verdict {
            status: Status::FailAccuracy,
            outcome: Some(compare::Outcome::Mismatch),
            notes: Vec::new(),
        };
        let mut v = verdict();
        downgrade_coarse_series_member(&out[0], &mut v); // 20 ns > nominal 10 ns
        assert_eq!(v.status, Status::Info);
        assert_eq!(
            v.outcome,
            Some(compare::Outcome::Mismatch),
            "the outcome is kept"
        );
        assert!(v.notes[0].contains("work-precision"));
        for c in &out[1..] {
            let mut v = verdict();
            downgrade_coarse_series_member(c, &mut v); // nominal step and finer stay judged
            assert_eq!(v.status, Status::FailAccuracy);
        }
        // a plain deck is never touched
        let mut v = verdict();
        downgrade_coarse_series_member(
            &one_case("t\n* bench.id: p\n* bench.group: accuracy\n.op\n", "p"),
            &mut v,
        );
        assert_eq!(v.status, Status::FailAccuracy);
    }

    #[test]
    fn a_series_gap_stays_open_while_any_member_confirms_it() {
        let mk = |id: &str, status: Status, base: &str| {
            let mut r = base_result(&one_case(
                &format!("t\n* bench.id: {id}\n* bench.group: accuracy\n.op\n"),
                id,
            ));
            r.status = status;
            r.known_gap = Some("fixed-step-accuracy".to_string());
            r.series = Some(SeriesRef {
                base: base.to_string(),
                index: 1,
                tstep: None,
            });
            r
        };
        let mut v = vec![
            mk("a__s1", Status::KnownGapConfirmed, "a"),
            mk("a__s2", Status::GapClosed, "a"),
            mk("b__s1", Status::GapClosed, "b"),
            mk("b__s2", Status::GapClosed, "b"),
        ];
        reconcile_series_gaps(&mut v);
        assert_eq!(v[1].status, Status::Pass);
        assert!(v[1].notes[0].contains("fixed-step-accuracy"));
        assert_eq!(v[0].status, Status::KnownGapConfirmed);
        // nothing confirms the gap of series b: it is closed for real
        assert_eq!(
            (v[2].status, v[3].status),
            (Status::GapClosed, Status::GapClosed)
        );
    }

    #[test]
    fn case_cwd_links_only_what_the_deck_can_reach() {
        let root = std::env::temp_dir().join(format!("rs-bench-cwd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src");
        let scratch = root.join("scratch");
        std::fs::create_dir_all(src.join("models")).unwrap();
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(src.join("A.LIB"), ".include models/b.lib\n").unwrap();
        std::fs::write(src.join("models").join("b.lib"), ".include c.lib\n").unwrap();
        std::fs::write(src.join("c.lib"), "* leaf\n").unwrap();
        std::fs::write(src.join("other.lib"), "* not reachable\n").unwrap();
        let mut case = one_case(
            "t\n* bench.id: cwd_case\n* bench.group: capability\n.include a.lib\n.op\n.end\n",
            "cwd_case",
        );
        case.src_dir = Some(src.clone());
        let cwd = case_cwd(&case, &scratch);
        assert_eq!(cwd, scratch.join("cwd"));
        let linked = |n: &str| cwd.join(n).symlink_metadata().is_ok();
        // the include chain is followed (case-insensitively, through a directory) ...
        assert!(linked("A.LIB") && linked("models") && linked("c.lib"));
        // ... and nothing else is linked
        assert!(!linked("other.lib"));
        // whatever a tool writes lands in the scratch directory, never in the source directory
        std::fs::write(cwd.join("b3v33check.log"), "x").unwrap();
        assert!(!src.join("b3v33check.log").exists());
        // a second call reuses the directory
        assert_eq!(case_cwd(&case, &scratch), cwd);
        // generated decks have no source directory and run in the scratch directory itself
        case.src_dir = None;
        assert_eq!(case_cwd(&case, &scratch), scratch);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_failure_reason_prefers_the_refusal_over_side_notes() {
        let mut r = base_result(&one_case(
            "t\n* bench.id: why\n* bench.group: robustness\n.op\n.end\n",
            "why",
        ));
        r.notes = vec![
            "rustspice printed a caught panic message on stderr".to_string(),
            "rustspice refused: exit 1: Error: SingularMatrix".to_string(),
        ];
        assert!(report::fail_reason(&r).starts_with("rustspice refused"));
        r.failures.push("v(1): norm_err too large".to_string());
        assert_eq!(report::fail_reason(&r), "v(1): norm_err too large");
        r.failures.clear();
        r.notes.remove(1);
        assert!(report::fail_reason(&r).starts_with("rustspice printed"));
    }

    // ---- E2: performance gaps, signatures, RSS bookkeeping -----------------------------------------------

    fn timing(
        e2e_ng: f64,
        e2e_rs: f64,
        solve_ng: f64,
        solve_rs: f64,
        noisy_ng: bool,
    ) -> TimingResult {
        let spread = |c: f64, noisy: bool| -> Vec<f64> {
            if noisy {
                vec![c * 0.5, c, c * 1.8, c * 0.6, c * 1.5]
            } else {
                vec![c; 5]
            }
        };
        let mk = |name: &str, e2e: Vec<f64>, sol: f64| ContenderTiming {
            name: name.to_string(),
            e2e_us: summarize(&e2e),
            analysis_us: summarize(&[sol; 5]),
            ..Default::default()
        };
        let contenders = vec![
            mk(
                "ngspice_sparse",
                spread(e2e_ng * 1.5, false),
                solve_ng * 2.0,
            ),
            mk("ngspice_klu", spread(e2e_ng, noisy_ng), solve_ng),
            mk("rustspice", spread(e2e_rs, false), solve_rs),
        ];
        let headline = headline(&contenders);
        TimingResult {
            noisy: contenders
                .iter()
                .any(|c| c.e2e_us.as_ref().is_some_and(|s| s.noisy)),
            contenders,
            headline,
            ..Default::default()
        }
    }

    #[test]
    fn a_performance_gap_is_judged_on_the_end_to_end_speedup_never_on_solve_only() {
        // the D8 case: solve-only 1.4x (rustspice "wins") but end to end 0.4x because of the symbolic setup
        let t = timing(29_000.0, 71_000.0, 22_000.0, 15_000.0, false);
        assert!(t.headline.solve_median.unwrap() > 1.0 && t.headline.e2e_median.unwrap() < 1.0);
        let (st, notes) = judge_perf_gap(Status::Pass, Some(&t), "measured", "faer-nonlinear-slow");
        assert_eq!(st, Status::KnownGapConfirmed);
        assert!(
            notes[0].contains("end-to-end median speedup 0.41x")
                && notes[0].contains("solve-only 1.47x"),
            "{notes:?}"
        );
        // genuinely faster end to end, quiet timing: closed
        let t = timing(30_000.0, 20_000.0, 22_000.0, 10_000.0, false);
        let (st, notes) = judge_perf_gap(Status::Pass, Some(&t), "measured", "faer-nonlinear-slow");
        assert_eq!(st, Status::GapClosed);
        assert!(
            notes[0].contains("closed") && notes[0].contains("1.50x"),
            "{notes:?}"
        );
        // parity band (1.0 .. 1.1): not closed
        let t = timing(21_000.0, 20_000.0, 1.0, 1.0, false);
        let (st, notes) = judge_perf_gap(Status::Info, Some(&t), "measured", "faer-nonlinear-slow");
        assert_eq!(st, Status::KnownGapConfirmed);
        assert!(
            notes[0].contains("below the 1.1x closing threshold"),
            "{notes:?}"
        );
        // fast enough but noisy: not closed either
        let t = timing(30_000.0, 20_000.0, 1.0, 1.0, true);
        assert!(t.noisy);
        let (st, notes) = judge_perf_gap(Status::Pass, Some(&t), "measured", "faer-nonlinear-slow");
        assert_eq!(st, Status::KnownGapConfirmed);
        assert!(notes[0].contains("noisy"), "{notes:?}");
        // the exact threshold closes (>= 1.1)
        let t = timing(22_000.0, 20_000.0, 1.0, 1.0, false);
        assert_eq!(
            judge_perf_gap(Status::Pass, Some(&t), "measured", "x").0,
            Status::GapClosed
        );
    }

    #[test]
    fn a_performance_gap_without_timing_is_unverified_and_failures_are_not_masked() {
        for (timing_status, t) in [
            ("disabled", None),
            ("unstable", Some(TimingResult::default())),
        ] {
            let (st, notes) = judge_perf_gap(
                Status::Pass,
                t.as_ref(),
                timing_status,
                "faer-nonlinear-slow",
            );
            assert_eq!(st, Status::Unverified, "{timing_status}");
            assert!(notes[0].contains("not judged") && notes[0].contains(timing_status));
        }
        // an accuracy / robustness failure is not covered by a performance slug
        for f in [
            Status::FailAccuracy,
            Status::FailRobustness,
            Status::Panic,
            Status::FailSilent,
        ] {
            let (st, notes) = judge_perf_gap(f, None, "skipped_status", "faer-nonlinear-slow");
            assert_eq!(st, f);
            assert!(notes[0].contains("performance-only"));
        }
        // a timeout is the extreme of slow: the gap is confirmed
        assert_eq!(
            judge_perf_gap(
                Status::Timeout,
                None,
                "skipped_status",
                "faer-nonlinear-slow"
            )
            .0,
            Status::KnownGapConfirmed
        );
        // other statuses pass through unchanged
        assert_eq!(
            judge_perf_gap(Status::NgspiceError, None, "x", "faer-nonlinear-slow").0,
            Status::NgspiceError
        );
    }

    #[test]
    fn failure_signatures_ignore_numbers_but_not_the_message() {
        assert_eq!(
            line_shape("Error: node n17 has 3 connections at 1.5e-9"),
            "error: node n# has # connections at #"
        );
        assert_eq!(
            line_shape("  thread 'main' panicked at crates/x/src/lu.rs:123:45: index out of bounds: the len is 5 but the index is 9 "),
            "thread 'main' panicked at crates/x/src/lu.rs:#:#: index out of bounds: the len is # but the index is #"
        );
        assert_ne!(
            line_shape("Error: SingularMatrix at row 3"),
            line_shape("Error: ConvergenceFailure at row 3")
        );
        assert_eq!(line_shape("0x7ffd12 and 42"), "# and #");
        assert!(line_shape(&"x".repeat(500)).len() <= 110);
    }

    #[test]
    fn artifacts_are_capped_per_failure_signature_not_per_family() {
        let t = TmpDir::new("sig-cap").unwrap();
        let mut ctx = Ctx {
            args: test_args(t.path()),
            exe: PathBuf::from("/bin/true"),
            tmp: TmpDir::new("sig-cap-tmp").unwrap(),
            lim: Limits {
                timeout: Duration::from_secs(5),
                max_rss_kib: None,
                pin_cpu: None,
            },
            ng_lim: Limits {
                timeout: Duration::from_secs(5),
                max_rss_kib: None,
                pin_cpu: None,
            },
            ng_installed: true,
            ng_reason: None,
            ref_cache: None,
            artifacts: t.path().join("art"),
            retained: HashMap::new(),
            artifact_budget: MAX_ARTIFACT_BYTES,
            reps: 5,
        };
        let mut kept: BTreeMap<&str, usize> = BTreeMap::new();
        // 10 mutants die with a "common" message, 2 with a different one, in the SAME family
        for (i, msg) in (0..12).map(|i| {
            (
                i,
                if i < 10 {
                    "panicked at lu.rs: index 17 out of range"
                } else {
                    "Error: stack overflow in subckt 4"
                },
            )
        }) {
            let mut c = one_case(
                &format!("t\n* bench.id: mut{i}\n* bench.group: robustness\n.op\n"),
                &format!("mut{i}"),
            );
            c.origin = Origin::Mutant;
            c.family = "mutants".to_string();
            let mut r = base_result(&c);
            let dir = ctx.tmp.path().join(&c.id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("rs.stderr"), format!("{msg}\nmore\n")).unwrap();
            let v = compare::Verdict {
                status: Status::Panic,
                outcome: Some(compare::Outcome::Panic),
                notes: Vec::new(),
            };
            finish_case(&mut ctx, &c, &mut r, &dir, NgSolver::Sparse, v);
            let k = if i < 10 { "common" } else { "other" };
            *kept.entry(k).or_default() += usize::from(r.artifacts.is_some());
            assert!(r.signature.as_deref().unwrap().starts_with("panic|"));
        }
        assert_eq!(kept["common"], ARTIFACTS_PER_SIGNATURE, "{kept:?}");
        assert_eq!(
            kept["other"], 2,
            "a different defect keeps its own artifacts: {kept:?}"
        );
        // static cases are never capped
        let mut kept_static = 0;
        for i in 0..6 {
            let c = one_case(
                &format!("t\n* bench.id: st{i}\n* bench.group: robustness\n.op\n"),
                &format!("st{i}"),
            );
            let mut r = base_result(&c);
            let dir = ctx.tmp.path().join(&c.id);
            std::fs::create_dir_all(&dir).unwrap();
            let v = compare::Verdict {
                status: Status::FailRobustness,
                outcome: None,
                notes: Vec::new(),
            };
            finish_case(&mut ctx, &c, &mut r, &dir, NgSolver::Sparse, v);
            kept_static += usize::from(r.artifacts.is_some());
            assert!(r.signature.is_none());
        }
        assert_eq!(kept_static, 6);
    }

    #[test]
    fn rss_report_checks_the_estimate_against_rustspice_only() {
        let c = one_case(
            "t\n* bench.id: rss\n* bench.group: performance\n.op\n",
            "rss",
        );
        let mut r = base_result(&c);
        note_rss(&mut r, "rustspice", Some(20 * 1024));
        note_rss(&mut r, "ngspice_sparse", Some(31 * 1024));
        note_rss(&mut r, "ngspice_sparse", Some(33 * 1024)); // a later run: the maximum is kept
        note_rss(&mut r, "ngspice_klu", None); // never reported as 0
        finalize_rss(&mut r, Some(30));
        let rep = r.rss.as_ref().unwrap();
        // the largest child overall is still reported ...
        assert_eq!(rep.max_child.as_deref(), Some("ngspice_sparse"));
        assert_eq!(rep.max_kib, Some(33 * 1024));
        // ... but ngspice's own 15-20 MiB start-up floor is not charged to the RustSpice estimate
        assert_eq!(rep.rustspice_kib, Some(20 * 1024));
        assert_eq!(rep.ngspice_max_kib, Some(33 * 1024));
        assert_eq!(rep.ngspice_max_child.as_deref(), Some("ngspice_sparse"));
        assert_eq!(rep.est_status, "within_estimate");
        let klu = rep
            .children
            .iter()
            .find(|c| c.child == "ngspice_klu")
            .unwrap();
        assert_eq!(
            (klu.peak_kib, klu.status.as_str(), klu.runs),
            (None, "unmeasured", 1)
        );
        let sparse = rep
            .children
            .iter()
            .find(|c| c.child == "ngspice_sparse")
            .unwrap();
        assert_eq!((sparse.peak_kib, sparse.runs), (Some(33 * 1024), 2));
        // negative: an estimate far below what RustSpice itself used is still flagged
        finalize_rss(&mut r, Some(10));
        assert_eq!(r.rss.as_ref().unwrap().est_status, "exceeds_estimate");
        finalize_rss(&mut r, None);
        assert_eq!(r.rss.as_ref().unwrap().est_status, "no_estimate");
        // nothing measured -> unmeasured, with null numbers
        let mut r2 = base_result(&c);
        note_rss(&mut r2, "rustspice", None);
        finalize_rss(&mut r2, Some(10));
        let rep = r2.rss.as_ref().unwrap();
        assert_eq!((rep.max_kib, rep.max_child.clone()), (None, None));
        assert_eq!(rep.est_status, "unmeasured");
        // ngspice measured but RustSpice not: the estimate is not judged on ngspice's number
        let mut r3 = base_result(&c);
        note_rss(&mut r3, "rustspice", None);
        note_rss(&mut r3, "ngspice_sparse", Some(500 * 1024));
        finalize_rss(&mut r3, Some(10));
        assert_eq!(r3.rss.as_ref().unwrap().est_status, "unmeasured");
    }

    // ----------------------------------------------------------------------------------------------------------
    // Engine hardening (E3c)
    // ----------------------------------------------------------------------------------------------------------

    fn timed_result(
        id: &str,
        fam: &str,
        size: u64,
        rs_kib: u64,
        ng_kib: u64,
        e2e_rs: f64,
    ) -> CaseResult {
        let c = one_case(
            &format!("t\n* bench.id: {id}\n* bench.group: performance\n.op\n"),
            id,
        );
        let mut r = base_result(&c);
        r.origin = "family".to_string();
        r.family = fam.to_string();
        r.size = Some(size);
        r.status = Status::Pass;
        r.timing = Some(timing(100.0, e2e_rs, 50.0, 40.0, false));
        r.timing_status = "measured".to_string();
        note_rss(&mut r, "rustspice", Some(rs_kib));
        note_rss(&mut r, "ngspice_sparse", Some(ng_kib));
        finalize_rss(&mut r, None);
        r
    }

    #[test]
    fn walk_never_follows_directory_symlinks_and_reports_what_it_cannot_read() {
        use std::os::unix::fs::symlink;
        let t = TmpDir::new("walk-links").unwrap();
        let root = t.path().join("cases");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(root.join("a/d.sp"), "t\n.op\n").unwrap();
        // a link back to the root and a link to the directory itself: following them never ends
        symlink(&root, root.join("a/up")).unwrap();
        symlink(".", root.join("self")).unwrap();
        // a deck-named link to a deck is a deck, a dangling one is an error and not a silently missing deck
        symlink(root.join("a/d.sp"), root.join("alias.sp")).unwrap();
        symlink(root.join("missing.sp"), root.join("dangling.sp")).unwrap();
        let mut w = Walked::default();
        walk(&root, 0, &mut w);
        assert_eq!(w.files, vec![root.join("a/d.sp"), root.join("alias.sp")]);
        assert_eq!(w.skipped_links.len(), 2, "{:?}", w.skipped_links);
        assert!(
            w.errors.iter().any(|e| e.contains("dangling")),
            "{:?}",
            w.errors
        );
        // the loader turns the unreadable entry into a usage error instead of a smaller green run
        let e = load_cases(&test_args(&root), Scale::Small, 1).unwrap_err();
        assert!(e.contains("dangling"), "{e}");
    }

    #[test]
    fn walk_has_a_depth_cap() {
        let t = TmpDir::new("walk-depth").unwrap();
        let mut p = t.path().to_path_buf();
        for i in 0..(MAX_WALK_DEPTH + 3) {
            p = p.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("deep.sp"), "t\n.op\n").unwrap();
        let mut w = Walked::default();
        walk(t.path(), 0, &mut w);
        assert!(w.files.is_empty(), "{:?}", w.files);
        assert!(
            w.errors.iter().any(|e| e.contains("nested deeper")),
            "{:?}",
            w.errors
        );
    }

    #[test]
    fn a_missing_or_empty_cases_directory_is_an_error_not_a_green_run_of_the_generated_decks() {
        let e = load_cases(
            &test_args(Path::new("/nonexistent-rustspice-cases/x")),
            Scale::Small,
            1,
        )
        .unwrap_err();
        assert!(e.contains("is not a directory"), "{e}");
        let t = TmpDir::new("empty-cases").unwrap();
        let e = load_cases(&test_args(t.path()), Scale::Small, 1).unwrap_err();
        assert!(e.contains("no benchmark decks"), "{e}");
        // only an explicit request runs the generated decks alone
        let mut a = test_args(Path::new("/nonexistent-rustspice-cases/x"));
        a.families_only = true;
        assert!(!load_cases(&a, Scale::Small, 1).unwrap().is_empty());
    }

    #[test]
    fn a_failed_scratch_write_is_an_error_naming_the_path() {
        let t = TmpDir::new("write-fail").unwrap();
        let bad = Path::new("/nonexistent-rustspice-dir/deck.sp");
        let e = write_file(bad, "x").unwrap_err();
        assert!(
            e.starts_with("scratch write failed")
                && e.contains("/nonexistent-rustspice-dir/deck.sp"),
            "{e}"
        );
        assert!(write_file(&t.path().join("ok.sp"), "x").is_ok());
        // the secondary-file helper records the loss once
        let c = one_case("t\n* bench.id: w\n* bench.group: robustness\n.op\n", "w");
        let mut r = base_result(&c);
        note_write(&mut r, Err(e.clone()));
        note_write(&mut r, Err(e));
        assert_eq!(r.notes.len(), 1, "{:?}", r.notes);
        note_write(&mut r, Ok(()));
        assert_eq!(r.notes.len(), 1);
    }

    fn test_ctx(root: &Path, exe: &str) -> Ctx {
        let lim = Limits {
            timeout: Duration::from_secs(10),
            max_rss_kib: None,
            pin_cpu: None,
        };
        Ctx {
            args: test_args(root),
            exe: PathBuf::from(exe),
            tmp: TmpDir::new("ctx-tmp").unwrap(),
            lim: lim.clone(),
            ng_lim: lim,
            ng_installed: true,
            ng_reason: None,
            ref_cache: None,
            artifacts: root.join("art"),
            retained: HashMap::new(),
            artifact_budget: MAX_ARTIFACT_BYTES,
            reps: 5,
        }
    }

    #[test]
    fn a_scratch_directory_that_cannot_be_written_makes_the_case_unverified_never_a_pass() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return; // root ignores directory permissions
        }
        let t = TmpDir::new("ro-scratch").unwrap();
        let mut ctx = test_ctx(t.path(), "/bin/true");
        // a loud_error deck is the one an empty deck would "pass": both tools refuse an empty file
        let c = one_case(
            "t\n* bench.id: ro\n* bench.group: robustness\n* bench.expect: loud_error\nR1 a b\n.op\n",
            "ro",
        );
        std::fs::set_permissions(ctx.tmp.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let r = run_case(&mut ctx, &c);
        std::fs::set_permissions(ctx.tmp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(r.status, Status::Unverified, "{:?}", r.notes);
        assert!(
            r.notes
                .iter()
                .any(|n| n.starts_with("scratch write failed")),
            "{:?}",
            r.notes
        );
        // remembered as a harness failure: the run exits 2 whatever the other cases say
        assert!(
            r.harness_failure
                .as_deref()
                .is_some_and(|m| m.starts_with("scratch write failed")),
            "{:?}",
            r.harness_failure
        );
    }

    #[test]
    fn every_case_origin_removes_its_scratch_directory_on_an_early_return() {
        let t = TmpDir::new("scratch-guard").unwrap();
        // RustSpice that exits 0 without a rawfile: `run_rustspice` ends the case early (after the directory and the
        // deck were written); before the guard the directory stayed in RAM-backed /dev/shm for the rest of the run
        for (origin, id) in [
            (Origin::Static, "g_static"),
            (Origin::Fuzz, "g_fuzz"),
            (Origin::Mutant, "g_mutant"),
            (Origin::Family, "g_family"),
        ] {
            let mut ctx = test_ctx(t.path(), "/bin/true");
            let mut c = one_case(
                &format!("t\n* bench.id: {id}\n* bench.group: robustness\n* bench.tags: ngspice_unbounded\n.op\n"),
                id,
            );
            c.origin = origin;
            let dir = case_scratch_dir(&ctx, &c);
            let r = run_case(&mut ctx, &c);
            assert_eq!(r.status, Status::Unverified, "{id}: {:?}", r.notes);
            assert!(
                !dir.exists(),
                "{id}: scratch directory {} was kept",
                dir.display()
            );
        }
        // the guard itself, also on a panic
        let d = t.path().join("guarded");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("big.raw"), "x").unwrap();
        let caught = std::panic::catch_unwind(|| {
            let _g = CaseScratch(d.clone());
            panic!("boom");
        });
        assert!(caught.is_err());
        assert!(!d.exists());
    }

    /// A RustSpice that exits 0 without a rawfile, or exits non-zero without saying why, is not "refusing the deck":
    /// with `expect: loud_error` the old harness called both a pass.
    #[test]
    fn a_rustspice_exit_without_output_or_message_is_never_a_refusal() {
        let t = TmpDir::new("rs-anomaly").unwrap();
        for (exe, needle) in [
            ("/bin/true", "exited 0 but wrote no rawfile"),
            ("/bin/false", "empty stderr"),
        ] {
            let mut ctx = test_ctx(t.path(), exe);
            let c = one_case(
                "t\n* bench.id: anomaly\n* bench.group: robustness\n* bench.expect: loud_error\n* bench.tags: ngspice_unbounded\nR1 a b\n.op\n",
                "anomaly",
            );
            let r = run_case(&mut ctx, &c);
            assert_eq!(r.status, Status::Unverified, "{exe}: {:?}", r.notes);
            assert!(
                r.notes.iter().any(|n| n.contains(needle)),
                "{exe}: {:?}",
                r.notes
            );
        }
        // a real refusal (exit 1 with a message) still passes a `loud_error` deck
        let sh = t.path().join("refuse.sh");
        std::fs::write(
            &sh,
            "#!/bin/sh\necho 'Error: Resistor requires two nodes and a value' >&2\nexit 1\n",
        )
        .unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut ctx = test_ctx(t.path(), sh.to_str().unwrap());
        let c = one_case(
            "t\n* bench.id: refusal\n* bench.group: robustness\n* bench.expect: loud_error\n* bench.tags: ngspice_unbounded\nR1 a b\n.op\n",
            "refusal",
        );
        let r = run_case(&mut ctx, &c);
        assert_eq!(r.status, Status::Pass, "{:?}", r.notes);
    }

    #[test]
    fn a_fallback_reference_never_backs_a_numeric_failure() {
        let v = |s| compare::Verdict {
            status: s,
            outcome: None,
            notes: Vec::new(),
        };
        for s in [Status::FailAccuracy, Status::KnownGapConfirmed] {
            let mut x = v(s);
            demote_on_fallback_reference("fell_back", Some(false), &mut x);
            assert_eq!(x.status, Status::Unverified, "{s:?}");
            assert!(x.notes[0].contains("fell_back"), "{:?}", x.notes);
        }
        // negatives: a converged reference, a pass, a refusal and a case without a numeric verdict are untouched
        let mut x = v(Status::FailAccuracy);
        demote_on_fallback_reference("converged_tran", Some(false), &mut x);
        assert_eq!(x.status, Status::FailAccuracy);
        let mut x = v(Status::Pass);
        demote_on_fallback_reference("fell_back", Some(true), &mut x);
        assert_eq!(x.status, Status::Pass);
        let mut x = v(Status::FailSilent);
        demote_on_fallback_reference("fell_back", Some(false), &mut x);
        assert_eq!(x.status, Status::FailSilent);
        let mut x = v(Status::Panic);
        demote_on_fallback_reference("fell_back", None, &mut x);
        assert_eq!(x.status, Status::Panic);
    }

    #[test]
    fn numeric_arguments_are_validated_before_anything_runs() {
        let t = TmpDir::new("args").unwrap();
        let ok = test_args(t.path());
        assert!(validate_numeric_args(&ok).is_ok());
        let bad = |f: &dyn Fn(&mut SuiteArgs)| {
            let mut a = test_args(t.path());
            f(&mut a);
            validate_numeric_args(&a).unwrap_err()
        };
        // `--max-rss-mib 0` killed every child at once and the report blamed RustSpice for "unbounded memory growth"
        assert!(bad(&|a| a.max_rss_mib = 0).contains("--max-rss-mib"));
        assert!(bad(&|a| a.max_rss_mib = MIN_RSS_CAP_MIB - 1).contains("--max-rss-mib"));
        assert!(bad(&|a| a.case_timeout = 0).contains("--case-timeout"));
        assert!(bad(&|a| a.case_timeout = MAX_CASE_TIMEOUT_S + 1).contains("--case-timeout"));
        assert!(bad(&|a| a.reps = 0).contains("--reps"));
        assert!(bad(&|a| a.reps = MAX_REPS + 1).contains("--reps"));
        assert!(bad(&|a| a.fuzz_count = Some(MAX_FUZZ_COUNT + 1)).contains("--fuzz-count"));
        // a CPU that does not exist: the old harness still reported "children pinned to CPU 999"
        assert!(bad(&|a| a.pin_cpu = Some(999)).contains("--pin-cpu"));
        assert!(bad(&|a| a.pin_cpu = Some(5000)).contains("--pin-cpu"));
        let mut a = test_args(t.path());
        a.max_rss_mib = MIN_RSS_CAP_MIB;
        a.fuzz_count = Some(MAX_FUZZ_COUNT);
        assert!(validate_numeric_args(&a).is_ok());
    }

    #[test]
    fn timing_needs_successful_samples_not_attempts() {
        let mut t = timing(100.0, 400.0, 50.0, 200.0, false);
        // every contender has five samples: judged
        assert_eq!(
            judge_perf_gap(Status::Pass, Some(&t), "measured", "faer-nonlinear-slow").0,
            Status::KnownGapConfirmed
        );
        // one ngspice sample (the other repetitions failed) is no measurement
        t.contenders[1].e2e_us = summarize(&[100.0]);
        let (s, n) = judge_perf_gap(Status::Pass, Some(&t), "unstable", "faer-nonlinear-slow");
        assert_eq!(s, Status::Unverified);
        assert!(
            n[0].contains("too few successful") && n[0].contains("ngspice_klu 1"),
            "{n:?}"
        );
        // a contender without any sample counts as zero
        t.contenders[1].e2e_us = None;
        let (s, n) = judge_perf_gap(Status::Pass, Some(&t), "unstable", "faer-nonlinear-slow");
        assert_eq!(s, Status::Unverified);
        assert!(n[0].contains("ngspice_klu 0"), "{n:?}");
        // `timing_status` is `measured` only when every contender reached the minimum
        let mut t = timing(100.0, 400.0, 50.0, 200.0, false);
        t.min_samples = MIN_TIMING_SAMPLES - 1;
        assert_eq!(timing_status_of(&t), "unstable");
        t.min_samples = MIN_TIMING_SAMPLES;
        assert_eq!(timing_status_of(&t), "measured");
        t.contenders.clear();
        assert_eq!(timing_status_of(&t), "unstable");
    }

    #[test]
    fn a_deck_that_selects_klu_itself_is_recognised() {
        let plain = one_case("t\n* bench.id: k0\n* bench.group: performance\n.op\n", "k0");
        assert!(!deck_requests_klu(&plain));
        let klu = one_case(
            "t\n* bench.id: k1\n* bench.group: performance\n.options klu\n.op\n",
            "k1",
        );
        assert!(deck_requests_klu(&klu));
        let opt = one_case(
            "t\n* bench.id: k2\n* bench.group: performance\n* bench.ng.options: klu\n.op\n",
            "k2",
        );
        assert!(deck_requests_klu(&opt));
    }

    #[test]
    fn scaling_never_fits_the_static_pile_and_fits_memory_per_unknown() {
        let mut cases: Vec<CaseResult> = Vec::new();
        // static decks with a size: an arbitrary mix of circuits, no meaningful exponent
        for (i, sz) in [10u64, 100, 1000, 5000].iter().enumerate() {
            let mut r = timed_result(&format!("st{i}"), "static", *sz, 10_000, 20_000, 100.0);
            r.origin = "static".to_string();
            cases.push(r);
        }
        // a real family: peak RSS grows as size^1.0 (1 KiB per unknown on top of nothing)
        for sz in [100u64, 1000, 10_000] {
            cases.push(timed_result(
                &format!("fam{sz}"),
                "ladder",
                sz,
                sz * 2,
                sz * 3,
                100.0,
            ));
        }
        let out = build_scaling(&cases);
        assert_eq!(
            out.len(),
            1,
            "{:?}",
            out.iter().map(|s| &s.family).collect::<Vec<_>>()
        );
        let f = &out[0];
        assert_eq!(f.family, "ladder");
        let e = f.fit_rss.rs.exponent.expect("RSS fit");
        assert!((e - 1.0).abs() < 1e-9, "{e}");
        assert_eq!(f.points[0].rs_kib_per_unknown, Some(2.0));
        assert_eq!(f.points[1].ng_kib_per_unknown, Some(3.0));
        // a family without RSS data gets a status, never a made-up exponent
        let mut bare = timed_result("b0", "bare", 10, 1, 1, 100.0);
        bare.rss = None;
        let mut bare2 = timed_result("b1", "bare", 100, 1, 1, 100.0);
        bare2.rss = None;
        let mut bare3 = timed_result("b2", "bare", 1000, 1, 1, 100.0);
        bare3.rss = None;
        let out = build_scaling(&[bare, bare2, bare3]);
        assert_eq!(out[0].fit_rss.rs.exponent, None);
        assert_ne!(out[0].fit_rss.rs.status, "ok");
    }

    #[test]
    fn slower_than_ngspice_lists_measured_quiet_non_gap_cases_slowest_first() {
        let mk = |id: &str, e2e_rs: f64, noisy_ng: bool| -> CaseResult {
            let mut r = timed_result(id, "f", 10, 1000, 1000, e2e_rs);
            r.timing = Some(timing(100.0, e2e_rs, 50.0, 40.0, noisy_ng));
            r
        };
        let mut cases = vec![
            mk("fast", 50.0, false),
            mk("slow_half", 200.0, false),
            mk("slow_quarter", 400.0, false),
            mk("slow_noisy", 400.0, true),
        ];
        let mut gap = mk("slow_gap", 400.0, false);
        gap.known_gap = Some("faer-nonlinear-slow".to_string());
        let mut unstable = mk("slow_unstable", 400.0, false);
        unstable.timing_status = "unstable".to_string();
        cases.push(gap);
        cases.push(unstable);
        let (slower, noisy) = build_slower(&cases);
        let ids: Vec<&str> = slower.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["slow_quarter", "slow_half"], "{ids:?}");
        assert!(slower[0].speedup_e2e < slower[1].speedup_e2e && slower[1].speedup_e2e < 1.0);
        assert_eq!(noisy, vec!["slow_noisy".to_string()]);
        assert!(slower[0].rs_e2e_us.is_some() && slower[0].ng_e2e_us.is_some());
    }

    #[test]
    fn the_start_up_self_test_rejects_an_ngspice_that_cannot_simulate() {
        use std::os::unix::fs::PermissionsExt;
        let t = TmpDir::new("selftest").unwrap();
        let fake = |name: &str, body: &str| -> String {
            let p = t.path().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p.display().to_string()
        };
        // answers `--version` but never simulates
        let dead = fake(
            "dead",
            "case \"$1\" in --version) echo '** ngspice-99';; *) echo boom >&2; exit 1;; esac",
        );
        let e = ngspice_selftest(&dead, t.path()).unwrap_err();
        assert!(e.contains("start-up check"), "{e}");
        // exits 0 and writes nothing (`quit` in a startup file does exactly this)
        let silent = fake("silent", "exit 0");
        assert!(ngspice_selftest(&silent, t.path())
            .unwrap_err()
            .contains("start-up check"));
        // a program that is not ngspice at all
        assert!(ngspice_selftest("/bin/true", t.path()).is_err());
        assert!(ngspice_selftest("/nonexistent/ngspice", t.path()).is_err());
        // the real one (when installed) passes, and leaves nothing behind
        if std::process::Command::new("ngspice")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            ngspice_selftest("ngspice", t.path()).unwrap();
            assert!(!t.path().join("ngspice-selftest").exists());
        }
    }

    fn sample_report(cases: Vec<CaseResult>, ng_installed: bool) -> SuiteReport {
        let summary = summarize_counts(&cases, ng_installed);
        let scaling = build_scaling(&cases);
        let (slower, slower_noisy) = build_slower(&cases);
        let d = meta::Meta::defaults("x");
        SuiteReport {
            schema: 1,
            generated_unix: 0,
            host: HostInfo {
                cpu: "test".to_string(),
                threads: 1,
                loadavg_start: None,
                loadavg_end: None,
                pin_cpu: None,
                tmp_on_tmpfs: true,
                pin_status: "not_requested".to_string(),
            },
            ngspice: NgInfo {
                version: Some("ngspice-47".to_string()),
                path: "ngspice".to_string(),
                installed: ng_installed,
                solvers_tested: Vec::new(),
                unusable_reason: None,
            },
            rustspice: RsInfo {
                version: "0".to_string(),
                git: None,
                dirty: None,
                build_profile: "release".to_string(),
                source_dir: None,
            },
            tolerances: Tolerances {
                tol_rel: d.tol_rel,
                tol_abs: d.tol_abs,
                tol_norm: d.tol_norm,
                floor: d.floor,
                tight_reference: TIGHT_OPTIONS.to_string(),
                tran_reference: TRAN_REF_OPTIONS.to_string(),
                noisy_threshold: NOISY_THRESHOLD,
            },
            args: Vec::new(),
            scale: "small".to_string(),
            seed: 1,
            startup_floor: None,
            cases,
            series: Vec::new(),
            fuzz: Vec::new(),
            scaling,
            slower_than_ngspice: slower,
            slower_than_ngspice_noisy: slower_noisy,
            teeth: None,
            corpus: None,
            summary,
        }
    }

    fn quick_case(id: &str, status: Status, notes: &[&str]) -> CaseResult {
        let c = one_case(
            &format!("t\n* bench.id: {id}\n* bench.group: accuracy\n.op\n"),
            id,
        );
        let mut r = base_result(&c);
        r.status = status;
        r.notes = notes.iter().map(|s| s.to_string()).collect();
        r
    }

    #[test]
    fn the_verdict_line_and_the_markdown_headline_name_every_unverified_case() {
        let cases = vec![
            quick_case("ok1", Status::Pass, &[]),
            quick_case(
                "odd",
                Status::Unverified,
                &[
                    "invalid case metadata; not run",
                    "line 4: bench.expect `maybe`",
                ],
            ),
            quick_case("odd2", Status::Unverified, &[]),
        ];
        let rep = sample_report(cases, true);
        // the JSON verdict vocabulary stays PASS | FAIL | UNVERIFIED, the new fields carry the detail
        assert_eq!(rep.summary.verdict, "PASS");
        assert_eq!(rep.summary.unverified_count, 2);
        assert!(
            rep.summary
                .verdict_line
                .starts_with("PASS (but 2 unverified"),
            "{}",
            rep.summary.verdict_line
        );
        assert_eq!(rep.summary.unverified[0].id, "odd");
        assert!(rep.summary.unverified[0].reason.contains("bench.expect"));
        assert_eq!(rep.summary.unverified[1].reason, "no reason recorded");
        let md = report::to_markdown(&rep);
        let head = md.split("## 2.").next().unwrap();
        assert!(head.contains("PASS (but 2 unverified"), "{head}");
        assert!(
            head.contains("`odd`") && head.contains("`maybe`") && head.contains("`odd2`"),
            "{head}"
        );
        // a run without any unverified case says plain PASS
        let clean = sample_report(vec![quick_case("ok1", Status::Pass, &[])], true);
        assert_eq!(clean.summary.verdict_line, "PASS");
        assert!(!report::to_markdown(&clean).contains("Unverified cases"));
        // all unverified, ngspice usable: UNVERIFIED, never PASS
        let none = sample_report(vec![quick_case("odd", Status::Unverified, &["x"])], true);
        assert_eq!(none.summary.verdict, "UNVERIFIED");
        // a failure is a FAIL whatever else is unverified
        let fail = sample_report(
            vec![
                quick_case("f", Status::FailAccuracy, &[]),
                quick_case("odd", Status::Unverified, &["x"]),
            ],
            true,
        );
        assert_eq!(fail.summary.verdict, "FAIL");
        assert!(
            fail.summary
                .verdict_line
                .starts_with("FAIL (1 failing, 1 unverified"),
            "{}",
            fail.summary.verdict_line
        );
    }

    #[test]
    fn the_known_gaps_table_shows_what_rustspice_did_not_only_the_authors_note() {
        let mut a = quick_case("gap_a", Status::KnownGapConfirmed, &[]);
        a.known_gap = Some("clamp-rail".to_string());
        a.gap_note = Some("the author says: loud refusal".to_string());
        a.outcome = Some("panic".to_string());
        a.rustspice = Some(RsRunInfo {
            class: "panic".to_string(),
            message: Some("thread main panicked at lu.rs".to_string()),
            wall_us: None,
            rss_kib: None,
            rss_status: "measured_vmhwm".to_string(),
            rss_poll_kib: None,
            rss_self_kib: None,
            termination: "rust_panic_exit_101".to_string(),
            stats: None,
            caught_panic_noise: false,
        });
        let mut b = quick_case("gap_b", Status::KnownGapConfirmed, &[]);
        b.known_gap = Some("clamp-rail".to_string());
        b.outcome = Some("loud_error".to_string());
        let md = report::to_markdown(&sample_report(vec![a, b], true));
        let row = md
            .lines()
            .find(|l| l.starts_with("| `clamp-rail`"))
            .expect("slug row");
        // the outcome class per case and the MEASURED message ahead of the labelled author note
        assert!(row.contains("loud_error=1, panic=1"), "{row}");
        assert!(
            row.contains(
                "thread main panicked at lu.rs (author note: the author says: loud refusal)"
            ),
            "{row}"
        );
    }

    #[test]
    fn the_report_has_a_slower_than_ngspice_section_and_a_memory_per_unknown_table() {
        let mut slow = timed_result("slow_case", "ladder", 100, 4000, 9000, 400.0);
        slow.timing = Some(timing(100.0, 400.0, 50.0, 40.0, false));
        let rep = sample_report(
            vec![
                slow,
                timed_result("fam_a", "fam", 10, 2000, 3000, 50.0),
                timed_result("fam_b", "fam", 100, 4000, 6000, 50.0),
                timed_result("fam_c", "fam", 1000, 8000, 12000, 50.0),
            ],
            true,
        );
        let md = report::to_markdown(&rep);
        let sec = md
            .split("### Slower than ngspice (end to end)")
            .nth(1)
            .expect("section")
            .split("\n## 7.")
            .next()
            .unwrap();
        assert!(sec.contains("slow_case"), "{sec}");
        assert!(!sec.contains("fam_a"), "{sec}");
        assert_eq!(rep.slower_than_ngspice.len(), 1);
        assert!(md.contains("peak RSS"), "scaling table has an RSS row");
        assert!(md.contains("KiB/unknown"), "memory per unknown table");
        // negative: nothing slower, the section says so instead of vanishing
        let md2 = report::to_markdown(&sample_report(
            vec![timed_result("fast", "fam", 10, 2000, 3000, 50.0)],
            true,
        ));
        assert!(md2.contains("No timed case with a measured"), "{md2}");
    }

    #[test]
    fn the_failing_cases_reason_is_what_rustspice_did() {
        let mut c = quick_case(
            "p",
            Status::Panic,
            &["ngspice crashed once and was retried (SIGSEGV)"],
        );
        c.rustspice = Some(RsRunInfo {
            class: "panic".to_string(),
            message: Some("index out of bounds in lu.rs".to_string()),
            wall_us: None,
            rss_kib: None,
            rss_status: "measured_vmhwm".to_string(),
            rss_poll_kib: None,
            rss_self_kib: None,
            termination: "rust_panic_exit_101".to_string(),
            stats: None,
            caught_panic_noise: false,
        });
        assert_eq!(report::fail_reason(&c), "index out of bounds in lu.rs");
        // a timeout without a message says whose clock ran out
        let t = quick_case("t", Status::Timeout, &["ngspice finished in 2 s"]);
        assert_eq!(
            report::fail_reason(&t),
            "rustspice hit the wall-clock limit"
        );
    }

    /// Developer tool, not a check: write the decks of the generated fuzz and mutant cases of a run to a directory
    /// so a failure can be reproduced outside the harness (the engine itself keeps only failing cases):
    /// `RS_DUMP_DIR=/dev/shm/x RS_CASES=benchmarks/cases RS_SEED=1 RS_COUNT=100 cargo test dump_generated_decks -- --ignored`
    #[test]
    #[ignore = "developer tool, needs RS_DUMP_DIR"]
    fn dump_generated_decks_for_triage() {
        let Ok(out) = std::env::var("RS_DUMP_DIR") else {
            return;
        };
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let mut args = test_args(Path::new(&get("RS_CASES", "benchmarks/cases")));
        args.no_fuzz = false;
        args.seed = get("RS_SEED", "1").parse().unwrap();
        let count: usize = get("RS_COUNT", "100").parse().unwrap();
        let cases = load_cases(&args, Scale::Small, count).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let mut n = 0;
        for c in cases
            .iter()
            .filter(|c| matches!(c.origin, Origin::Fuzz | Origin::Mutant))
        {
            let bytes = c.raw.clone().unwrap_or_else(|| c.deck.clone().into_bytes());
            std::fs::write(Path::new(&out).join(format!("{}.sp", c.id)), bytes).unwrap();
            n += 1;
        }
        eprintln!("wrote {n} decks to {out}");
    }
}
