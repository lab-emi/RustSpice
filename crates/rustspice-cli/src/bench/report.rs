//! Result types (the JSON schema of SUITE_SPEC section 8) and the JSON / Markdown renderers.
//!
//! Rule 4: a quantity that was not measured is `null` plus a `*_status` string, never `0`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use super::compare::{OracleSummary, ProbeResult, Status};
use super::rustsim::RunStats;
use super::stats::{EqualError, Fit, Summary};
use super::teeth::{TeethResult, TeethSummary};

// ---------------------------------------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct HostInfo {
    pub cpu: String,
    pub threads: usize,
    pub loadavg_start: Option<f64>,
    pub loadavg_end: Option<f64>,
    /// The CPU the children were pinned to: `null` when no pinning was requested OR when it failed (see `pin_status`).
    pub pin_cpu: Option<usize>,
    pub tmp_on_tmpfs: bool,
    /// `not_requested` | `applied` | `failed: <n> child start(s) could not be pinned to CPU <c>`.
    pub pin_status: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NgInfo {
    pub version: Option<String>,
    pub path: String,
    /// `true` only for a usable ngspice (`--version` exits 0 and names itself ngspice).
    pub installed: bool,
    pub solvers_tested: Vec<String>,
    /// Why ngspice is not usable (`null` when it is).
    pub unusable_reason: Option<String>,
}

/// The binary under test: `git` / `dirty` describe the source tree this binary was built from (the repository of
/// the crate directory it was compiled in), `null` when that tree is not available at run time.
#[derive(Clone, Debug, Serialize)]
pub struct RsInfo {
    pub version: String,
    pub git: Option<String>,
    pub dirty: Option<bool>,
    pub build_profile: String,
    /// The directory `git` / `dirty` were read from (`null` when it does not exist any more).
    pub source_dir: Option<String>,
}

/// The benchmark corpus (`--cases`): its own repository state, kept apart from the binary's.
#[derive(Clone, Debug, Serialize)]
pub struct CorpusInfo {
    pub path: String,
    pub git: Option<String>,
    pub dirty: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tolerances {
    pub tol_rel: f64,
    pub tol_abs: f64,
    pub tol_norm: f64,
    pub floor: f64,
    pub tight_reference: String,
    pub tran_reference: String,
    pub noisy_threshold: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Floors {
    pub status: String,
    pub reps: usize,
    pub ngspice_us: Option<Summary>,
    pub rustspice_us: Option<Summary>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ContenderTiming {
    /// `ngspice_sparse` | `ngspice_klu` | `rustspice`
    pub name: String,
    pub e2e_us: Option<Summary>,
    /// ngspice `Total analysis time` / rustspice `--stats analysis_us` (solve only).
    pub analysis_us: Option<Summary>,
    /// ngspice: elapsed - analysis (elapsed has 1 ms resolution); rustspice: parse + compile.
    pub setup_us: Option<Summary>,
    /// Non-solve cost per run: `e2e - solve` (process start-up, parse, setup, rawfile write, exit).
    pub overhead_us: Option<Summary>,
    pub parse_us: Option<Summary>,
    pub compile_us: Option<Summary>,
    /// Peak RSS per repetition (`VmHWM` polled from `/proc`; RustSpice also its own final reading), in KiB.
    pub rss_kib: Option<Summary>,
    /// `measured_vmhwm` (every repetition has a value) | `partial` | `unmeasured`.
    pub rss_status: String,
    pub failed_reps: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SolverSpeedup {
    pub solver: String,
    pub e2e_median: Option<f64>,
    pub e2e_min: Option<f64>,
    pub solve_median: Option<f64>,
    pub solve_min: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Headline {
    /// median_ng / median_rs against the fastest ngspice solver (per metric); >1 means rustspice is faster.
    pub e2e_median: Option<f64>,
    pub e2e_min: Option<f64>,
    pub solve_median: Option<f64>,
    pub solve_min: Option<f64>,
    pub e2e_vs: Option<String>,
    pub solve_vs: Option<String>,
    pub per_solver: Vec<SolverSpeedup>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TimingResult {
    pub reps_requested: usize,
    /// Successful repetitions: the smallest number of end-to-end samples any contender produced (a failed run is not
    /// a sample).
    pub reps_done: usize,
    /// Repetitions that were started (failed runs included).
    pub reps_attempted: usize,
    /// Fewest end-to-end samples of any contender (`reps_done`'s source); judging needs at least 3.
    pub min_samples: usize,
    pub contenders: Vec<ContenderTiming>,
    pub headline: Headline,
    /// Execution order of each repetition (shows the interleaving).
    pub order_trace: Vec<String>,
    pub loadavg_before: Option<f64>,
    pub loadavg_after: Option<f64>,
    pub noisy: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AccuracySummary {
    pub pass: Option<bool>,
    pub err_rs: Option<f64>,
    pub err_ng: Option<f64>,
    pub within_ng_error: Option<bool>,
    pub interp_floor: Option<f64>,
    pub points_rs: Option<usize>,
    pub points_ref: Option<usize>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct NgRunInfo {
    pub solver: String,
    pub status: String,
    pub wall_us: Option<f64>,
    pub analysis_us: Option<f64>,
    pub elapsed_us: Option<f64>,
    /// Peak RSS of the correctness run in KiB (`VmHWM` from `/proc/<pid>/status`), `null` when unmeasured.
    pub rss_kib: Option<u64>,
    /// `measured_vmhwm` | `measured_tree_vmhwm` (wrapper with descendants) | `unmeasured` (the process ended before
    /// the first poll) | `not_run`.
    pub rss_status: String,
    /// The number is the maximum of `/proc` polls: a lower bound of the true peak (the last poll interval before the
    /// exit is not seen), unlike RustSpice's own final reading.
    pub rss_lower_bound: bool,
    /// The captured console output was cut or incomplete (head and tail kept).
    pub output_truncated: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RsRunInfo {
    /// `ok` | `loud_error` | `panic` | `timeout` | `not_run`
    pub class: String,
    pub message: Option<String>,
    pub wall_us: Option<f64>,
    /// Peak RSS in KiB: the larger of the polled `VmHWM` and the process's own final `--stats` reading.
    pub rss_kib: Option<u64>,
    /// `measured_vmhwm` | `vmhwm_poll_only` | `own_stats_only` | `unmeasured`.
    pub rss_status: String,
    pub rss_poll_kib: Option<u64>,
    pub rss_self_kib: Option<u64>,
    /// How the process ended: `exit_<n>` | `rust_panic_exit_101` | `signal_<NAME>` (a crash or an outside kill) |
    /// `killed_by_harness_rss_cap` (memory growth: the harness ended it) | `killed_by_harness_timeout` |
    /// `harness_io_failure` | `not_run`. `class` keeps the status vocabulary; this tells a Rust panic or abort from
    /// a kill at the `--max-rss-mib` cap.
    pub termination: String,
    pub stats: Option<RunStats>,
    /// stderr carries a caught panic message although the exit was clean (informational, not a panic).
    pub caught_panic_noise: bool,
}

/// Peak RSS of one child process of a case (maximum over every run of it: correctness, reference, timed reps).
#[derive(Clone, Debug, Serialize)]
pub struct ChildRss {
    /// `rustspice` | `ngspice_sparse` | `ngspice_klu` | `ngspice_ref`
    pub child: String,
    pub peak_kib: Option<u64>,
    /// `measured_vmhwm` | `unmeasured`
    pub status: String,
    pub runs: usize,
}

/// SUITE_SPEC Rule 9: the estimate `bench.est_rss_mib` is RustSpice's peak RSS and is compared with the RustSpice
/// child only (`rustspice_kib`); `max_child` / `max_kib` still name the largest child overall, and the ngspice peak is
/// reported separately (`ngspice_max_kib`).
#[derive(Clone, Debug, Serialize)]
pub struct RssReport {
    pub children: Vec<ChildRss>,
    pub max_child: Option<String>,
    pub max_kib: Option<u64>,
    pub est_mib: Option<u64>,
    /// `within_estimate` | `exceeds_estimate` | `no_estimate` | `unmeasured`. The estimate is RustSpice's peak RSS
    /// and is compared with `rustspice_kib` ONLY: ngspice has a 15-20 MiB start-up floor of its own, which made every
    /// small deck "exceed" it.
    pub est_status: String,
    /// Peak RSS of the RustSpice child (maximum over its runs), the number the estimate is compared with.
    pub rustspice_kib: Option<u64>,
    /// Largest peak RSS among the ngspice children, reported separately.
    pub ngspice_max_kib: Option<u64>,
    pub ngspice_max_child: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesRef {
    pub base: String,
    pub index: usize,
    pub tstep: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub group: String,
    pub family: String,
    pub origin: String,
    pub title: String,
    pub tags: Vec<String>,
    pub expect: String,
    pub known_gap: Option<String>,
    pub gap_note: Option<String>,
    pub status: Status,
    /// Set when the case could not be judged because the HARNESS environment failed (scratch file system, a child
    /// that could not be started, RustSpice's harness-I/O exit 74): the case is `unverified` and the run exits 2.
    pub harness_failure: Option<String>,
    pub outcome: Option<String>,
    pub analysis: Option<String>,
    pub size: Option<u64>,
    pub probes: Vec<ProbeResult>,
    /// `ok` | `error: ...` | `timeout` | `not installed` | `skipped_unbounded` (tag `ngspice_unbounded`) | `not_run`.
    pub ngspice_status: String,
    pub ref_status: String,
    pub ref_options: Option<String>,
    pub accuracy: Option<AccuracySummary>,
    pub oracle: Option<OracleSummary>,
    pub ngspice: Option<NgRunInfo>,
    pub rustspice: Option<RsRunInfo>,
    pub timing: Option<TimingResult>,
    pub timing_status: String,
    /// Peak RSS of every child the case started, and the Rule-9 check of `bench.est_rss_mib` against the RustSpice child.
    pub rss: Option<RssReport>,
    /// Mutation-sensitivity check (`--teeth`); `null` when the flag is off.
    pub teeth: Option<TeethResult>,
    /// Failure signature (status + shape of the first stderr line) of a failing fuzz / mutant case: the key of the
    /// per-signature artifact cap.
    pub signature: Option<String>,
    pub series: Option<SeriesRef>,
    pub failures: Vec<String>,
    pub notes: Vec<String>,
    pub artifacts: Option<String>,
    pub case_wall_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesPoint {
    pub id: String,
    pub tstep: Option<f64>,
    pub status: Status,
    pub err_rs: Option<f64>,
    pub err_ng: Option<f64>,
    pub rs_solve_us: Option<f64>,
    pub ng_solve_us: Option<f64>,
    pub rs_e2e_us: Option<f64>,
    pub ng_e2e_us: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesResult {
    pub base: String,
    pub title: String,
    pub points: Vec<SeriesPoint>,
    /// "time at equal error", solve-only; `ratio_b_over_a` = t_ngspice / t_rustspice.
    pub equal_error_solve: EqualError,
    pub equal_error_e2e: EqualError,
}

#[derive(Clone, Debug, Serialize)]
pub struct FuzzFailure {
    pub id: String,
    pub status: Status,
    pub note: String,
    pub artifact: Option<String>,
}

/// One distinct failure signature (status + shape of the first stderr line) of a fuzz / mutant family.
#[derive(Clone, Debug, Serialize)]
pub struct SignatureCount {
    pub signature: String,
    pub count: usize,
    /// How many of them kept their artifacts (at most `ARTIFACTS_PER_SIGNATURE`).
    pub artifacts_kept: usize,
    pub first_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FuzzSummary {
    pub family: String,
    pub count: usize,
    pub by_status: BTreeMap<String, usize>,
    /// Failing cases whose decks were kept under the artifacts directory: the first three of each distinct failure
    /// signature (at most 12 per family), so a common root cause cannot hide a different defect.
    pub first_failures: Vec<FuzzFailure>,
    /// Every distinct failure signature of the family with its case count.
    pub signatures: Vec<SignatureCount>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalingPoint {
    pub id: String,
    pub size: u64,
    pub ng_sparse_solve_us: Option<f64>,
    pub ng_klu_solve_us: Option<f64>,
    pub rs_solve_us: Option<f64>,
    pub ng_sparse_e2e_us: Option<f64>,
    pub ng_klu_e2e_us: Option<f64>,
    pub rs_e2e_us: Option<f64>,
    /// Peak RSS of each child over every run of the case (KiB); `null` when it was not measured.
    pub ng_sparse_rss_kib: Option<u64>,
    pub ng_klu_rss_kib: Option<u64>,
    pub rs_rss_kib: Option<u64>,
    /// Peak RSS per problem unknown (KiB / size) of RustSpice and of the larger ngspice child.
    pub rs_kib_per_unknown: Option<f64>,
    pub ng_kib_per_unknown: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalingFits {
    pub ng_sparse: Fit,
    pub ng_klu: Fit,
    pub rs: Fit,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalingResult {
    pub family: String,
    pub points: Vec<ScalingPoint>,
    /// Fits of solve-only time versus problem size.
    pub fit: ScalingFits,
    /// Fits of end-to-end subprocess time versus problem size.
    pub fit_e2e: ScalingFits,
    /// Fits of peak RSS versus problem size (`rs` = RustSpice, the other two ngspice); `status` says why a fit is
    /// missing.
    pub fit_rss: ScalingFits,
}

/// A case that could not be judged, with the reason that the report lists in its headline.
#[derive(Clone, Debug, Serialize)]
pub struct UnverifiedCase {
    pub id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CountSummary {
    pub counts: BTreeMap<String, usize>,
    pub by_group: BTreeMap<String, BTreeMap<String, usize>>,
    pub total: usize,
    /// `PASS` | `FAIL` | `UNVERIFIED` (every case unverified). A run with some unverified cases and no failure is
    /// `PASS` here, and `verdict_line` says so.
    pub verdict: String,
    /// The verdict as the stderr summary and the Markdown headline print it: `PASS (3 unverified)`.
    pub verdict_line: String,
    pub unverified_count: usize,
    pub unverified: Vec<UnverifiedCase>,
}

/// One timed case on which RustSpice is SLOWER than the faster ngspice solver end to end (measured, not noisy,
/// no performance known-gap slug): a performance weakness that no accuracy status shows.
#[derive(Clone, Debug, Serialize)]
pub struct SlowerCase {
    pub id: String,
    pub size: Option<u64>,
    /// median ngspice / median rustspice end to end against the faster solver; < 1.0 here.
    pub speedup_e2e: f64,
    pub vs: String,
    pub solve_speedup: Option<f64>,
    /// RustSpice's linear-solver tier as `--stats` reports it (`solver_kind`).
    pub rs_solver_kind: Option<String>,
    pub rs_e2e_us: Option<f64>,
    pub ng_e2e_us: Option<f64>,
    /// RustSpice compile (symbolic setup / JIT) and solve medians: where the time goes.
    pub rs_compile_us: Option<f64>,
    pub rs_parse_us: Option<f64>,
    pub rs_solve_us: Option<f64>,
    pub ng_solve_us: Option<f64>,
    pub known_gap: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuiteReport {
    pub schema: u32,
    pub generated_unix: u64,
    pub host: HostInfo,
    pub ngspice: NgInfo,
    pub rustspice: RsInfo,
    pub tolerances: Tolerances,
    pub args: Vec<String>,
    pub scale: String,
    pub seed: u64,
    pub startup_floor: Option<Floors>,
    pub cases: Vec<CaseResult>,
    pub series: Vec<SeriesResult>,
    pub fuzz: Vec<FuzzSummary>,
    pub scaling: Vec<ScalingResult>,
    /// Timed cases where RustSpice is slower than ngspice end to end, slowest first.
    pub slower_than_ngspice: Vec<SlowerCase>,
    /// Timed cases that LOOK slower but were excluded because the measurement is noisy.
    pub slower_than_ngspice_noisy: Vec<String>,
    /// Present when `--teeth` was given.
    pub teeth: Option<TeethSummary>,
    /// The corpus the cases were read from.
    pub corpus: Option<CorpusInfo>,
    pub summary: CountSummary,
}

pub fn summarize_counts(cases: &[CaseResult], any_ng: bool) -> CountSummary {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_group: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for s in Status::ALL {
        counts.insert(s.as_str().to_string(), 0);
    }
    for c in cases {
        *counts.entry(c.status.as_str().to_string()).or_default() += 1;
        *by_group
            .entry(c.group.clone())
            .or_default()
            .entry(c.status.as_str().to_string())
            .or_default() += 1;
    }
    let fails = cases.iter().filter(|c| c.status.is_fail()).count();
    let verdict = if fails > 0 {
        "FAIL"
    } else if !any_ng
        || cases
            .iter()
            .all(|c| matches!(c.status, Status::Unverified | Status::SkippedBudget))
    {
        "UNVERIFIED"
    } else {
        "PASS"
    };
    let unverified: Vec<UnverifiedCase> = cases
        .iter()
        .filter(|c| c.status == Status::Unverified)
        .map(|c| UnverifiedCase {
            id: c.id.clone(),
            reason: unverified_reason(c),
        })
        .collect();
    let n_unv = unverified.len();
    let verdict_line = match (verdict, fails, n_unv) {
        ("FAIL", f, 0) => format!("FAIL ({f} failing)"),
        ("FAIL", f, u) => format!("FAIL ({f} failing, {u} unverified)"),
        ("PASS", _, 0) => "PASS".to_string(),
        ("PASS", _, u) => format!("PASS (but {u} unverified: not judged, listed below)"),
        (v, _, _) => v.to_string(),
    };
    CountSummary {
        counts,
        by_group,
        total: cases.len(),
        verdict: verdict.to_string(),
        verdict_line,
        unverified_count: n_unv,
        unverified,
    }
}

/// Why a case is `unverified`: the notes that carry the reason (the first two, clipped).
pub fn unverified_reason(c: &CaseResult) -> String {
    let n: Vec<&str> = c
        .notes
        .iter()
        .map(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .take(2)
        .collect();
    if n.is_empty() {
        "no reason recorded".to_string()
    } else {
        short(&n.join("; "), 220)
    }
}

pub fn to_json(r: &SuiteReport) -> String {
    serde_json::to_string_pretty(r)
        .unwrap_or_else(|e| format!("{{\"schema\":1,\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------------------------------------

pub fn fmt_us(us: f64) -> String {
    if !us.is_finite() {
        return "n/a".to_string();
    }
    if us < 1.0 {
        format!("{:.2} µs", us)
    } else if us < 100.0 {
        format!("{:.1} µs", us)
    } else if us < 1000.0 {
        format!("{:.0} µs", us)
    } else if us < 100_000.0 {
        format!("{:.2} ms", us / 1e3)
    } else if us < 1e6 {
        format!("{:.0} ms", us / 1e3)
    } else {
        format!("{:.2} s", us / 1e6)
    }
}

fn fmt_opt_us(v: Option<f64>) -> String {
    v.map_or("n/a".to_string(), fmt_us)
}

fn fmt_e(v: Option<f64>) -> String {
    v.map_or("n/a".to_string(), |x| format!("{x:.2e}"))
}

fn fmt_x(v: Option<f64>) -> String {
    match v {
        Some(x) if x >= 100.0 => format!("{x:.0}x"),
        Some(x) if x >= 10.0 => format!("{x:.1}x"),
        Some(x) => format!("{x:.2}x"),
        None => "n/a".to_string(),
    }
}

fn fmt_mib(kib: Option<f64>) -> String {
    kib.map_or("n/a".to_string(), |k| format!("{:.1}", k / 1024.0))
}

fn med_min(s: &Option<Summary>) -> String {
    match s {
        Some(s) => format!(
            "{} / {}{}",
            fmt_us(s.median),
            fmt_us(s.min),
            if s.noisy { " (noisy)" } else { "" }
        ),
        None => "n/a".to_string(),
    }
}

fn esc(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn short(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}...")
    } else {
        t
    }
}

fn status_cols(by: &BTreeMap<String, usize>) -> String {
    let v: Vec<String> = Status::ALL
        .iter()
        .filter_map(|s| {
            by.get(s.as_str())
                .filter(|n| **n > 0)
                .map(|n| format!("{}={}", s.as_str(), n))
        })
        .collect();
    if v.is_empty() {
        "-".to_string()
    } else {
        v.join(", ")
    }
}

fn find<'a>(t: &'a TimingResult, name: &str) -> Option<&'a ContenderTiming> {
    t.contenders.iter().find(|c| c.name == name)
}

// ---------------------------------------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------------------------------------

pub fn to_markdown(r: &SuiteReport) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# RustSpice vs ngspice differential benchmark\n");
    section_verdict(&mut s, r);
    section_known_gaps(&mut s, r);
    section_capability(&mut s, r);
    section_accuracy(&mut s, r);
    section_teeth(&mut s, r);
    section_robustness(&mut s, r);
    section_performance(&mut s, r);
    section_scaling(&mut s, r);
    section_methodology(&mut s, r);
    section_repro(&mut s, r);
    s
}

fn section_verdict(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 1. Verdict\n");
    let _ = writeln!(
        s,
        "**{}** — {} cases, scale `{}`, seed {}, ngspice `{}`, rustspice {} ({}){}\n",
        r.summary.verdict_line,
        r.summary.total,
        r.scale,
        r.seed,
        r.ngspice
            .version
            .clone()
            .unwrap_or_else(|| "not installed".to_string()),
        r.rustspice.version,
        r.rustspice
            .git
            .clone()
            .unwrap_or_else(|| "git n/a".to_string()),
        if r.rustspice.dirty == Some(true) {
            ", dirty tree"
        } else {
            ""
        }
    );
    if r.rustspice.build_profile != "release" {
        let _ = writeln!(
            s,
            "> WARNING: rustspice was built with profile `{}`; timing numbers are not representative.\n",
            r.rustspice.build_profile
        );
    }
    let closed: Vec<&CaseResult> = r
        .cases
        .iter()
        .filter(|c| c.status == Status::GapClosed)
        .collect();
    if !closed.is_empty() {
        let _ = writeln!(
            s,
            "> **Known gaps now CLOSED (update benchmarks/README.md and drop `bench.known_gap`):**"
        );
        for c in closed {
            let _ = writeln!(
                s,
                "> - `{}` ({})",
                c.id,
                c.known_gap.clone().unwrap_or_default()
            );
        }
        s.push('\n');
    }
    let _ = writeln!(s, "| group | cases | statuses |\n|---|---|---|");
    for (g, by) in &r.summary.by_group {
        let n: usize = by.values().sum();
        let _ = writeln!(s, "| {g} | {n} | {} |", status_cols(by));
    }
    let _ = writeln!(
        s,
        "| **all** | **{}** | {} |\n",
        r.summary.total,
        status_cols(&r.summary.counts)
    );
    if !r.summary.unverified.is_empty() {
        let _ = writeln!(
            s,
            "Unverified cases ({}) — NOT judged, the verdict above says nothing about them:\n",
            r.summary.unverified.len()
        );
        for u in r.summary.unverified.iter().take(60) {
            let _ = writeln!(s, "- `{}` — {}", u.id, esc(&u.reason));
        }
        if r.summary.unverified.len() > 60 {
            let _ = writeln!(
                s,
                "- ... {} more (see JSON `summary.unverified`)",
                r.summary.unverified.len() - 60
            );
        }
        s.push('\n');
    }
    let other: Vec<&CaseResult> = r
        .cases
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                Status::NgspiceError | Status::BothError | Status::SkippedBudget
            )
        })
        .collect();
    if !other.is_empty() {
        let _ = writeln!(
            s,
            "Cases without a numeric verdict (ngspice failed or the case was skipped, {}):\n",
            other.len()
        );
        for c in other.iter().take(30) {
            let _ = writeln!(
                s,
                "- `{}` **{}** — {}",
                c.id,
                c.status.as_str(),
                esc(&short(c.notes.first().map_or("", |x| x.as_str()), 160))
            );
        }
        if other.len() > 30 {
            let _ = writeln!(s, "- ... {} more (see JSON)", other.len() - 30);
        }
        s.push('\n');
    }
    let fails: Vec<&CaseResult> = r.cases.iter().filter(|c| c.status.is_fail()).collect();
    if !fails.is_empty() {
        let _ = writeln!(s, "Failing cases ({}):\n", fails.len());
        for c in fails.iter().take(40) {
            let _ = writeln!(
                s,
                "- `{}` **{}** — {}",
                c.id,
                c.status.as_str(),
                esc(&short(fail_reason(c), 160))
            );
        }
        if fails.len() > 40 {
            let _ = writeln!(s, "- ... {} more (see JSON)", fails.len() - 40);
        }
        s.push('\n');
    }
}

/// One line saying why a case failed: its first failure, else the refusal note (ahead of side notes such as caught
/// panic noise), else its first note.
pub(super) fn fail_reason(c: &CaseResult) -> &str {
    // what RustSpice did comes first for a panic / timeout / refusal: the first NOTE of such a case is often about
    // ngspice (a crash that was retried, a failed reference run)
    if matches!(
        c.status,
        Status::Panic | Status::Timeout | Status::FailRobustness
    ) {
        if let Some(m) = c.rustspice.as_ref().and_then(|x| x.message.as_deref()) {
            return m;
        }
        if c.status == Status::Timeout {
            return "rustspice hit the wall-clock limit";
        }
    }
    c.failures
        .first()
        .or_else(|| c.notes.iter().find(|n| n.starts_with("rustspice refused")))
        .or(c.notes.first())
        .map_or("", |x| x.as_str())
}

fn section_known_gaps(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 2. Known gaps (by slug)\n");
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for c in &r.cases {
        if matches!(c.status, Status::KnownGapConfirmed | Status::GapClosed) {
            by.entry(c.known_gap.clone().unwrap_or_else(|| "?".to_string()))
                .or_default()
                .push(c);
        }
    }
    if by.is_empty() {
        let _ = writeln!(s, "No known-gap cases were run.\n");
        return;
    }
    let _ = writeln!(
        s,
        "| slug | cases | RustSpice outcome | observed (first confirmed case) |\n|---|---|---|---|"
    );
    for (slug, cs) in &by {
        let confirmed: Vec<&&CaseResult> = cs
            .iter()
            .filter(|c| c.status == Status::KnownGapConfirmed)
            .collect();
        let closed = cs.len() - confirmed.len();
        let ids: Vec<String> = cs
            .iter()
            .take(6)
            .map(|c| {
                format!(
                    "`{}`{}",
                    c.id,
                    if c.status == Status::GapClosed {
                        " (closed)"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        let more = if cs.len() > 6 {
            format!(" +{}", cs.len() - 6)
        } else {
            String::new()
        };
        // what RustSpice actually DID, per case (`outcome`), so a slug that absorbs a panic where a refusal was
        // documented shows up as a different class
        let mut by_outcome: BTreeMap<String, usize> = BTreeMap::new();
        for c in cs {
            *by_outcome
                .entry(c.outcome.clone().unwrap_or_else(|| "n/a".to_string()))
                .or_default() += 1;
        }
        let outcomes = by_outcome
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ");
        // measured evidence first (RustSpice's message, the failed comparison), the author's note last and labelled
        let observed = confirmed
            .first()
            .map(|c| {
                let measured = c
                    .rustspice
                    .as_ref()
                    .and_then(|r| r.message.clone())
                    .or_else(|| c.failures.first().cloned())
                    .or_else(|| c.notes.first().cloned());
                match (measured, c.gap_note.as_deref()) {
                    (Some(m), Some(n)) => format!("{m} (author note: {n})"),
                    (Some(m), None) => m,
                    (None, Some(n)) => format!("(author note) {n}"),
                    (None, None) => String::new(),
                }
            })
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "| `{slug}` | {} confirmed / {} closed: {}{} | {} | {} |",
            confirmed.len(),
            closed,
            ids.join(", "),
            more,
            esc(&outcomes),
            esc(&short(&observed, 160))
        );
    }
    s.push('\n');
}

fn section_capability(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 3. Capability matrix\n");
    let cs: Vec<&CaseResult> = r.cases.iter().filter(|c| c.group == "capability").collect();
    if cs.is_empty() {
        let _ = writeln!(s, "No capability cases were run.\n");
        return;
    }
    let mut by_outcome: BTreeMap<String, usize> = BTreeMap::new();
    for c in &cs {
        *by_outcome
            .entry(c.outcome.clone().unwrap_or_else(|| "n/a".to_string()))
            .or_default() += 1;
    }
    let _ = writeln!(
        s,
        "Outcomes: {}\n",
        by_outcome
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(
        s,
        "| case | expect | outcome | status | detail |\n|---|---|---|---|---|"
    );
    let mut rows = cs.clone();
    rows.sort_by_key(|c| (c.known_gap.is_none(), c.known_gap.clone(), c.id.clone()));
    for c in rows {
        let detail = c
            .failures
            .first()
            .or(c.notes.first())
            .map_or(String::new(), |x| short(x, 110));
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {} | {} |",
            c.id,
            c.expect,
            c.outcome.clone().unwrap_or_else(|| "n/a".to_string()),
            c.status.as_str(),
            esc(&detail)
        );
    }
    s.push('\n');
}

fn worst_probe(c: &CaseResult) -> Option<&ProbeResult> {
    c.probes
        .iter()
        .filter(|p| !p.vacuous && p.rs_err.is_some())
        .max_by(|a, b| {
            a.rs_err
                .partial_cmp(&b.rs_err)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn section_accuracy(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 4. Accuracy\n");
    let cs: Vec<&CaseResult> = r
        .cases
        .iter()
        .filter(|c| {
            c.group == "accuracy"
                && c.series.is_none()
                && !matches!(c.status, Status::SkippedBudget)
        })
        .collect();
    if cs.is_empty() {
        let _ = writeln!(s, "No accuracy cases were run.\n");
    } else {
        let _ = writeln!(
            s,
            "Errors are `max |rustspice - reference|` (op/dc: volts or amps; tran: normalised to the reference swing). `ng as-is` is default-option ngspice against the same reference; `oracle` is the independent f64 solution.\n"
        );
        let _ = writeln!(
            s,
            "| case | analysis | worst probe | RS err | ng as-is err | RS vs oracle | status |\n|---|---|---|---|---|---|---|"
        );
        for c in cs {
            let wp = worst_probe(c);
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} | {} | {} | {} |",
                c.id,
                c.analysis.clone().unwrap_or_default(),
                wp.map_or("-".to_string(), |p| p.name.clone()),
                fmt_e(wp.and_then(|p| p.rs_err)),
                fmt_e(wp.and_then(|p| p.ng_err)),
                fmt_e(c.oracle.as_ref().and_then(|o| o.rs_max_err)),
                c.status.as_str()
            );
        }
        s.push('\n');
    }
    if !r.series.is_empty() {
        let _ = writeln!(s, "### Work-precision series\n");
        let _ = writeln!(
            s,
            "Fixed-grid rustspice versus adaptive ngspice (`tmax = tstep`). Error is the normalised max error against the converged reference; \"ratio\" = ngspice time / rustspice time at equal error (>1: rustspice faster), interpolated log-log inside the error range covered by both curves.\n"
        );
        for sr in &r.series {
            let _ = writeln!(s, "**`{}`** — {}\n", sr.base, esc(&sr.title));
            let _ = writeln!(
                s,
                "| tstep | RS err | ng err | RS solve | ng solve | RS e2e | ng e2e |\n|---|---|---|---|---|---|---|"
            );
            for p in &sr.points {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    p.tstep.map_or("n/a".to_string(), |t| format!("{t:.3e}")),
                    fmt_e(p.err_rs),
                    fmt_e(p.err_ng),
                    fmt_opt_us(p.rs_solve_us),
                    fmt_opt_us(p.ng_solve_us),
                    fmt_opt_us(p.rs_e2e_us),
                    fmt_opt_us(p.ng_e2e_us)
                );
            }
            s.push('\n');
            for (label, ee) in [
                ("solve-only", &sr.equal_error_solve),
                ("end-to-end", &sr.equal_error_e2e),
            ] {
                match (&ee.status[..], ee.range) {
                    ("ok", Some((lo, hi))) => {
                        let _ = writeln!(
                            s,
                            "Time at equal error, {label} (error range {lo:.1e} .. {hi:.1e}):\n"
                        );
                        let _ = writeln!(s, "| error | RS | ngspice | ratio |\n|---|---|---|---|");
                        for l in &ee.levels {
                            let _ = writeln!(
                                s,
                                "| {:.2e} | {} | {} | {} |",
                                l.err,
                                fmt_us(l.t_a),
                                fmt_us(l.t_b),
                                fmt_x(Some(l.ratio_b_over_a))
                            );
                        }
                        s.push('\n');
                    }
                    (st, _) => {
                        let rng = |x: Option<(f64, f64)>| {
                            x.map_or("n/a".to_string(), |(a, b)| format!("{a:.1e}..{b:.1e}"))
                        };
                        let _ = writeln!(
                            s,
                            "Time at equal error, {label}: **{st}** (RS error range {}, ngspice {}).\n",
                            rng(ee.a_range),
                            rng(ee.b_range)
                        );
                    }
                }
            }
        }
    }
}

fn section_teeth(s: &mut String, r: &SuiteReport) {
    let Some(t) = &r.teeth else { return };
    let _ = writeln!(s, "### Mutation sensitivity (`--teeth`)\n");
    let _ = writeln!(
        s,
        "RustSpice alone was re-run on copies of every numeric `pass` deck with the first, a middle and the last value-carrying element (R/C/L/V/I/G value, MOS `W`) scaled by {:.2}, and each mutant was compared with the ORIGINAL ngspice data under the case's own tolerances. `ok`: at least one mutation is detected; `blunt`: every evaluated mutation still passes, i.e. the comparison cannot see the circuit; `skipped`: the deck could not be mutated or no mutated run gave a verdict; `n/a`: not a numeric pass. Cases with `value` indirection (`{{expr}}`, parameters, `.subckt` bodies, continuation lines), zero values, and elements that cannot influence the analysis (C/L in DC analyses, the swept source of `.dc`) are not mutation targets. `status` is never changed.\n",
        t.factor
    );
    let _ = writeln!(
        s,
        "| ok | blunt | skipped | n/a |\n|---|---|---|---|\n| {} | {} | {} | {} |\n",
        t.ok, t.blunt, t.skipped, t.n_a
    );
    if t.blunt > 0 {
        let _ = writeln!(
            s,
            "Blunt passes by reason: {}\n",
            reason_counts(&t.blunt_by_reason)
        );
        let mut by: BTreeMap<&str, Vec<&super::teeth::BluntCase>> = BTreeMap::new();
        for c in &t.blunt_cases {
            by.entry(c.reason_class.as_str()).or_default().push(c);
        }
        for (class, cs) in by {
            let _ = writeln!(s, "- **{class}** ({}):", cs.len());
            for c in cs.iter().take(60) {
                let _ = writeln!(
                    s,
                    "  - `{}` ({}) — {}",
                    c.id,
                    c.group,
                    esc(&short(&c.reason, 150))
                );
            }
            if cs.len() > 60 {
                let _ = writeln!(s, "  - ... {} more (see JSON)", cs.len() - 60);
            }
        }
        s.push('\n');
    }
    if t.ok_sources_only > 0 {
        let _ = writeln!(
            s,
            "`ok` only through a source: {} cases pass the check because the V/I mutation is detected, while every passive / MOS mutation survived (the comparison sees the amplitude, not the circuit behind it), by the reason those survived: {}\n",
            t.ok_sources_only,
            reason_counts(&t.sources_only_by_reason)
        );
        let mut by: BTreeMap<&str, Vec<&super::teeth::BluntCase>> = BTreeMap::new();
        for c in &t.sources_only_cases {
            by.entry(c.reason_class.as_str()).or_default().push(c);
        }
        for (class, cs) in by {
            let ids: Vec<String> = cs.iter().take(60).map(|c| format!("`{}`", c.id)).collect();
            let more = if cs.len() > 60 {
                format!(" +{}", cs.len() - 60)
            } else {
                String::new()
            };
            let _ = writeln!(
                s,
                "- **{class}** ({}): {}{}",
                cs.len(),
                ids.join(", "),
                more
            );
        }
        s.push('\n');
    }
    if t.skipped > 0 {
        let _ = writeln!(
            s,
            "Skipped by reason: {}\n",
            reason_counts(&t.skipped_by_reason)
        );
    }
}

fn reason_counts(m: &BTreeMap<String, usize>) -> String {
    m.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn section_robustness(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 5. Robustness\n");
    let cs: Vec<&CaseResult> = r
        .cases
        .iter()
        .filter(|c| c.group == "robustness" && c.origin == "static")
        .collect();
    if cs.is_empty() {
        let _ = writeln!(s, "No static robustness cases were run.\n");
    } else {
        let _ = writeln!(
            s,
            "| case | expect | RustSpice | ngspice | status |\n|---|---|---|---|---|"
        );
        for c in cs {
            let rs = c.rustspice.as_ref().map_or("n/a".to_string(), |x| {
                format!(
                    "{}{}{}",
                    x.class,
                    x.message
                        .as_ref()
                        .map_or(String::new(), |m| format!(": {}", short(m, 70))),
                    if x.caught_panic_noise {
                        " [caught panic noise]"
                    } else {
                        ""
                    }
                )
            });
            let ng = c
                .ngspice
                .as_ref()
                .map_or("n/a".to_string(), |x| short(&x.status, 50));
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} | {} |",
                c.id,
                c.expect,
                esc(&rs),
                esc(&ng),
                c.status.as_str()
            );
        }
        s.push('\n');
    }
    if !r.fuzz.is_empty() {
        let _ = writeln!(s, "### Fuzz and mutation summary\n");
        let _ = writeln!(
            s,
            "| family | cases | statuses | first failures |\n|---|---|---|---|"
        );
        for f in &r.fuzz {
            let ff: Vec<String> = f
                .first_failures
                .iter()
                .map(|x| format!("`{}` ({})", x.id, x.status.as_str()))
                .collect();
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                f.family,
                f.count,
                status_cols(&f.by_status),
                if ff.is_empty() {
                    "-".to_string()
                } else {
                    ff.join(", ")
                }
            );
        }
        s.push('\n');
        let with_sigs: Vec<&FuzzSummary> =
            r.fuzz.iter().filter(|f| !f.signatures.is_empty()).collect();
        if !with_sigs.is_empty() {
            let _ = writeln!(
                s,
                "Failure signatures (status + shape of the first stderr line; the first 3 cases of each keep their artifacts):\n"
            );
            for f in with_sigs {
                for sg in &f.signatures {
                    let _ = writeln!(
                        s,
                        "- {}: {} x `{}` (first `{}`, {} kept)",
                        f.family,
                        sg.count,
                        esc(&short(&sg.signature, 120)),
                        sg.first_id,
                        sg.artifacts_kept
                    );
                }
            }
            s.push('\n');
        }
    }
}

fn section_performance(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 6. Performance\n");
    let timed: Vec<&CaseResult> = r.cases.iter().filter(|c| c.timing.is_some()).collect();
    if let Some(f) = &r.startup_floor {
        let _ = writeln!(
            s,
            "Process start-up floor (trivial `.op` deck, wall of `-b -r` / `run --raw`): ngspice {}, rustspice {} ({} reps, interleaved).\n",
            med_min(&f.ngspice_us),
            med_min(&f.rustspice_us),
            f.reps
        );
    }
    if timed.is_empty() {
        let _ = writeln!(
            s,
            "No timing was performed (`--no-timing`, or no timed cases ran).\n"
        );
        section_rss(s, r);
        return;
    }
    let _ = writeln!(
        s,
        "Times are `median / min` over interleaved repetitions. Speedup = median ngspice / median rustspice (>1: rustspice faster) against the FASTER ngspice solver for that metric; both ngspice solvers are shown.\n"
    );
    let _ = writeln!(
        s,
        "### End-to-end (subprocess wall: start-up + parse + setup + solve + write rawfile)\n"
    );
    let _ = writeln!(
        s,
        "| case | size | ng SPARSE | ng KLU | rustspice | speedup (median) | speedup (min) | vs |\n|---|---|---|---|---|---|---|---|"
    );
    for c in &timed {
        let t = c.timing.as_ref().unwrap();
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
            c.id,
            c.size.map_or("-".to_string(), |x| x.to_string()),
            med_min(&find(t, "ngspice_sparse").and_then(|x| x.e2e_us.clone())),
            med_min(&find(t, "ngspice_klu").and_then(|x| x.e2e_us.clone())),
            med_min(&find(t, "rustspice").and_then(|x| x.e2e_us.clone())),
            fmt_x(t.headline.e2e_median),
            fmt_x(t.headline.e2e_min),
            t.headline.e2e_vs.clone().unwrap_or_else(|| "-".to_string())
        );
    }
    s.push('\n');
    let _ = writeln!(s, "### Solve-only (ngspice `Total analysis time` vs rustspice `--stats analysis_us`) and overhead split\n");
    let _ = writeln!(
        s,
        "| case | ng SPARSE solve | ng KLU solve | RS solve | speedup (median) | speedup (min) | ng overhead | RS overhead | RS parse+compile | RSS ng / RS (MiB) | noisy |\n|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for c in &timed {
        let t = c.timing.as_ref().unwrap();
        let ng_best = ["ngspice_sparse", "ngspice_klu"]
            .iter()
            .filter_map(|n| find(t, n))
            .min_by(|a, b| {
                let am = a.e2e_us.as_ref().map_or(f64::INFINITY, |x| x.median);
                let bm = b.e2e_us.as_ref().map_or(f64::INFINITY, |x| x.median);
                am.partial_cmp(&bm).unwrap_or(std::cmp::Ordering::Equal)
            });
        let rs = find(t, "rustspice");
        let _ = writeln!(
            s,
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} / {} | {} |",
            c.id,
            med_min(&find(t, "ngspice_sparse").and_then(|x| x.analysis_us.clone())),
            med_min(&find(t, "ngspice_klu").and_then(|x| x.analysis_us.clone())),
            med_min(&rs.and_then(|x| x.analysis_us.clone())),
            fmt_x(t.headline.solve_median),
            fmt_x(t.headline.solve_min),
            ng_best
                .and_then(|x| x.overhead_us.as_ref())
                .map_or("n/a".to_string(), |x| fmt_us(x.median)),
            rs.and_then(|x| x.overhead_us.as_ref())
                .map_or("n/a".to_string(), |x| fmt_us(x.median)),
            rs.and_then(|x| x.setup_us.as_ref())
                .map_or("n/a".to_string(), |x| fmt_us(x.median)),
            fmt_mib(ng_best.and_then(|x| x.rss_kib.as_ref()).map(|x| x.median)),
            fmt_mib(rs.and_then(|x| x.rss_kib.as_ref()).map(|x| x.median)),
            if t.noisy { "yes" } else { "no" }
        );
    }
    let _ = writeln!(
        s,
        "\nOverhead = end-to-end wall minus solve time (process start-up, load, parse, setup, rawfile write, exit); for the faster ngspice solver and for rustspice. rustspice parse+compile comes from `--stats` (`compile` includes the Cranelift JIT / symbolic LU build). Loadavg before/after each timed case is in the JSON.\n"
    );
    section_slower(s, r);
    section_rss(s, r);
}

/// Timed cases where RustSpice loses end to end: weaknesses that do not fail an accuracy status.
fn section_slower(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "### Slower than ngspice (end to end)\n");
    if r.slower_than_ngspice.is_empty() {
        let _ = writeln!(
            s,
            "No timed case with a measured, non-noisy end-to-end median speedup below 1.0 against the faster ngspice solver (cases carrying a performance known-gap slug are judged by their own status).\n"
        );
    } else {
        let _ = writeln!(
            s,
            "{} timed case(s) where the median end-to-end time of RustSpice is above that of the faster ngspice solver (speedup < 1.0; measured, not noisy; no performance known-gap slug), slowest first. `compile` is RustSpice's own setup (symbolic factorisation / JIT) from `--stats`, `solve` its analysis time.\n",
            r.slower_than_ngspice.len()
        );
        let _ = writeln!(
            s,
            "| case | size | speedup | vs | RS solver | RS e2e | ng e2e | RS parse | RS compile | RS solve | ng solve |\n|---|---|---|---|---|---|---|---|---|---|---|"
        );
        for c in r.slower_than_ngspice.iter().take(60) {
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                c.id,
                c.size.map_or("-".to_string(), |x| x.to_string()),
                fmt_x(Some(c.speedup_e2e)),
                c.vs,
                c.rs_solver_kind
                    .clone()
                    .unwrap_or_else(|| "n/a".to_string()),
                fmt_opt_us(c.rs_e2e_us),
                fmt_opt_us(c.ng_e2e_us),
                fmt_opt_us(c.rs_parse_us),
                fmt_opt_us(c.rs_compile_us),
                fmt_opt_us(c.rs_solve_us),
                fmt_opt_us(c.ng_solve_us)
            );
        }
        if r.slower_than_ngspice.len() > 60 {
            let _ = writeln!(
                s,
                "\n... {} more (see JSON `slower_than_ngspice`)",
                r.slower_than_ngspice.len() - 60
            );
        }
        s.push('\n');
    }
    if !r.slower_than_ngspice_noisy.is_empty() {
        let _ = writeln!(
            s,
            "Excluded as noisy (the median is below 1.0 but the spread is too large to call it): {}.\n",
            r.slower_than_ngspice_noisy
                .iter()
                .take(20)
                .map(|x| format!("`{x}`"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// Rule 9: measured peak RSS (maximum over the children of a case) versus the `bench.est_rss_mib` estimate.
fn section_rss(s: &mut String, r: &SuiteReport) {
    let with_est: Vec<&CaseResult> = r
        .cases
        .iter()
        .filter(|c| c.rss.as_ref().is_some_and(|x| x.est_mib.is_some()))
        .collect();
    if with_est.is_empty() {
        return;
    }
    let over: Vec<&&CaseResult> = with_est
        .iter()
        .filter(|c| {
            c.rss
                .as_ref()
                .is_some_and(|x| x.est_status == "exceeds_estimate")
        })
        .collect();
    let unmeasured = with_est
        .iter()
        .filter(|c| c.rss.as_ref().is_some_and(|x| x.est_status == "unmeasured"))
        .count();
    let _ = writeln!(
        s,
        "### Peak RSS versus `bench.est_rss_mib` (Rule 9)\n\n{} cases carry an estimate; it is RustSpice's peak RSS and is compared with the RustSpice child only (ngspice has a 15-20 MiB start-up floor of its own and is reported separately in the JSON `rss`). {} exceed it by more than 25 %, {} could not be measured.\n",
        with_est.len(),
        over.len(),
        unmeasured
    );
    if !over.is_empty() {
        let _ = writeln!(
            s,
            "| case | est MiB | RustSpice MiB | ngspice max MiB |\n|---|---|---|---|"
        );
        for c in over.iter().take(40) {
            let x = c.rss.as_ref().unwrap();
            let _ = writeln!(
                s,
                "| `{}` | {} | {} | {} |",
                c.id,
                x.est_mib.map_or("n/a".to_string(), |e| e.to_string()),
                fmt_mib(x.rustspice_kib.map(|k| k as f64)),
                fmt_mib(x.ngspice_max_kib.map(|k| k as f64))
            );
        }
        if over.len() > 40 {
            let _ = writeln!(s, "\n... {} more (see JSON `rss`)", over.len() - 40);
        }
        s.push('\n');
    }
}

fn section_scaling(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 7. Scaling fits\n");
    if r.scaling.is_empty() {
        let _ = writeln!(s, "No family had enough timed size points.\n");
        return;
    }
    let fmt_fit = |f: &Fit| match (f.exponent, f.r2) {
        (Some(p), Some(r2)) => format!("{p:.2} (R²={r2:.3}, n={})", f.points),
        _ => format!("n/a ({})", f.status),
    };
    let _ = writeln!(
        s,
        "Exponent `p` in `time ~ size^p` and `peak RSS ~ size^p` (log-log least squares; needs >= 3 points). Only families with an explicit `family:<id>` tag (or a generated family) are fitted: unrelated static decks are never pooled.\n"
    );
    let _ = writeln!(
        s,
        "| family | points | metric | ng SPARSE | ng KLU | rustspice |\n|---|---|---|---|---|---|"
    );
    for sc in &r.scaling {
        let _ = writeln!(
            s,
            "| {} | {} | solve | {} | {} | {} |",
            sc.family,
            sc.points.len(),
            fmt_fit(&sc.fit.ng_sparse),
            fmt_fit(&sc.fit.ng_klu),
            fmt_fit(&sc.fit.rs)
        );
        let _ = writeln!(
            s,
            "| {} | {} | e2e | {} | {} | {} |",
            sc.family,
            sc.points.len(),
            fmt_fit(&sc.fit_e2e.ng_sparse),
            fmt_fit(&sc.fit_e2e.ng_klu),
            fmt_fit(&sc.fit_e2e.rs)
        );
        let _ = writeln!(
            s,
            "| {} | {} | peak RSS | {} | {} | {} |",
            sc.family,
            sc.points.len(),
            fmt_fit(&sc.fit_rss.ng_sparse),
            fmt_fit(&sc.fit_rss.ng_klu),
            fmt_fit(&sc.fit_rss.rs)
        );
    }
    s.push('\n');
    let _ = writeln!(
        s,
        "Peak RSS per problem unknown (KiB / size; ngspice = the larger of its two solvers):\n"
    );
    let _ = writeln!(
        s,
        "| family | case | size | RS peak MiB | RS KiB/unknown | ng peak MiB | ng KiB/unknown |\n|---|---|---|---|---|---|---|"
    );
    for sc in &r.scaling {
        for p in sc.points.iter().take(40) {
            let ng_peak = match (p.ng_sparse_rss_kib, p.ng_klu_rss_kib) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            let _ = writeln!(
                s,
                "| {} | `{}` | {} | {} | {} | {} | {} |",
                sc.family,
                p.id,
                p.size,
                fmt_mib(p.rs_rss_kib.map(|k| k as f64)),
                p.rs_kib_per_unknown
                    .map_or("n/a".to_string(), |x| format!("{x:.3}")),
                fmt_mib(ng_peak.map(|k| k as f64)),
                p.ng_kib_per_unknown
                    .map_or("n/a".to_string(), |x| format!("{x:.3}"))
            );
        }
    }
    s.push('\n');
}

fn section_methodology(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 8. Methodology and caveats\n");
    let _ = writeln!(
        s,
        "- Host: {} ({} threads); load average {} at start, {} at end{}. Timing ratios are only comparable within this session; the host is shared.",
        r.host.cpu,
        r.host.threads,
        r.host.loadavg_start.map_or("n/a".to_string(), |x| format!("{x:.1}")),
        r.host.loadavg_end.map_or("n/a".to_string(), |x| format!("{x:.1}")),
        match (r.host.pin_cpu, r.host.pin_status.as_str()) {
            (Some(c), _) => format!("; children pinned to CPU {c}"),
            (None, "not_requested") => String::new(),
            (None, st) => format!("; CPU pinning requested but NOT in effect ({st})"),
        }
    );
    let _ = writeln!(
        s,
        "- Temp files live on {}.",
        if r.host.tmp_on_tmpfs {
            "tmpfs (`/dev/shm`)"
        } else {
            "the system temp directory (not tmpfs)"
        }
    );
    let _ = writeln!(
        s,
        "- ngspice: `{}`; as-is runs use default options, `.options klu` selects KLU (default is SPARSE). Solvers tested: {}.",
        r.ngspice.version.clone().unwrap_or_else(|| "n/a".to_string()),
        r.ngspice.solvers_tested.join(", ")
    );
    let _ = writeln!(
        s,
        "- Accuracy is judged against a tightly converged ngspice reference (`.options {}`; transients: `{}` with `tmax = tstep/20`), not against default-tolerance ngspice, whose own error is reported separately. Independent f64 oracles are used where one exists.",
        r.tolerances.tight_reference, r.tolerances.tran_reference
    );
    let _ = writeln!(
        s,
        "- Default tolerances: op/dc |a-b| <= {:.0e} + {:.0e}*|ref|; tran max error / swing <= {:.0e} (floor {:.0e}). Probes whose reference is below the floor are vacuous and never count as a pass.",
        r.tolerances.tol_abs, r.tolerances.tol_rel, r.tolerances.tol_norm, r.tolerances.floor
    );
    let _ = writeln!(
        s,
        "- Timing: interleaved A/B (execution order rotates every repetition), the correctness run is the warm-up, `n` repetitions per case, noisy = stddev/median > {:.1}. Peak RSS is `VmHWM` of `/proc/<pid>/status` polled while the child runs (RustSpice also reports its own final reading in `--stats`), never `wait4` `ru_maxrss`, which is floored by the harness's own high-water mark; a child that ends before the first poll has `rss_status: unmeasured` and a `null` value.",
        r.tolerances.noisy_threshold
    );
    let _ = writeln!(
        s,
        "- Transient comparison: both waveforms are linearly interpolated onto the nominal output grid `k * tstep` (the instants at which a fixed-step simulator has samples) inside the window [first ngspice row, tstop]; `.tran` start times are honoured, a `tstep` >= `tstop` or a parameterised card uses the union of the ngspice and rustspice time rows. Series members coarser than the deck's own `.tran` step are work-precision points (`info` when they miss `tol.norm`)."
    );
    let _ = writeln!(
        s,
        "- Decks run with the working directory set to their own directory (relative `.include`/`.lib` targets resolve for both tools). A segfault of ngspice is retried once; a deterministic crash is `ngspice_error`. Tag `ngspice_unbounded` skips ngspice (`ngspice_status: skipped_unbounded`); a caught panic message with a clean exit is a refusal, only exit 101 or a signal is a panic."
    );
    let _ = writeln!(
        s,
        "- Scope: temperature 27 C (RustSpice is fixed at 300.15 K), `.options` are not honoured by RustSpice, RustSpice transients are fixed-step, one analysis per deck unless stated. A missing ngspice yields `unverified`, never a pass."
    );
    s.push('\n');
}

fn section_repro(s: &mut String, r: &SuiteReport) {
    let _ = writeln!(s, "## 9. Reproduction\n");
    let _ = writeln!(s, "```");
    let skip = r
        .args
        .iter()
        .position(|a| a == "bench-suite")
        .map_or(0, |i| i + 1);
    let _ = writeln!(s, "rustspice bench-suite {}", r.args[skip..].join(" "));
    let _ = writeln!(s, "```\n");
    let _ = writeln!(
        s,
        "Single case: `rustspice run <deck> --raw out.raw --stats out.json` against `ngspice -b -r ref.raw <deck>`; kept artifacts of non-passing cases are listed in the JSON (`artifacts`)."
    );
}
