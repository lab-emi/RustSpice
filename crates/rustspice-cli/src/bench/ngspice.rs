//! ngspice as a black-box executable: `ngspice -b -r <raw> <deck>`, log parsing, failure classification.
//! Nothing here depends on ngspice internals; only observable output (rawfile, console log, exit status).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::meta::NgSolver;
use super::rawfile::{self, Plot};
use super::rustsim::{run_guarded, ChildRun, Exit, Limits, SpawnError};

/// Facts extracted from the batch-mode console log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogInfo {
    /// `Total analysis time (seconds)`, microsecond resolution.
    pub analysis_s: Option<f64>,
    /// `Total elapsed time (seconds)`, 1 ms resolution.
    pub elapsed_s: Option<f64>,
    pub solver: Option<NgSolver>,
    /// Lines matching the failure patterns of SUITE_SPEC section 3 (`Warning:` lines excluded).
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

/// Failure messages of ngspice, matched at the START of a console line (after at most three `word:` prefixes such as
/// `doAnalyses: TRAN: `), on a word boundary. They used to be substring matches over the whole line, so a line that
/// merely ECHOED text (the raw-file path with a case id containing `error`, a `.tran` node-table row of a node named
/// `error_amp`, a scratch directory under such a name) made a good run `ngspice_error`.
const FAILURE_STARTS: &[&str] = &[
    "error",
    "fatal",
    "singular matrix",
    "no convergence",
    "timestep too small",
    "too many iterations",
    "simulation(s) aborted",
    "run simulation(s) aborted",
];

/// `low` (a lower-case line) starts with `pat` and the match ends on a word boundary.
fn starts_with_word(low: &str, pat: &str) -> bool {
    low.strip_prefix(pat).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    })
}

/// A lower-case line is a failure message when a failure start sits at its beginning or after up to three
/// `<word>:` prefixes.
fn is_failure_line(low: &str) -> bool {
    let mut t = low;
    for _ in 0..4 {
        if FAILURE_STARTS.iter().any(|p| starts_with_word(t, p)) {
            return true;
        }
        let Some(i) = t.find(':') else { return false };
        let head = &t[..i];
        if head.is_empty()
            || !head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        {
            return false;
        }
        t = t[i + 1..].trim_start();
    }
    false
}

/// A node-table row of the `.tran` / `.op` printout (`<name>   <number>`): the name is the user's, whatever it says.
fn is_table_row(t: &str) -> bool {
    let mut it = t.split_whitespace();
    match (it.next(), it.next(), it.next()) {
        (Some(a), Some(b), None) => !a.ends_with(':') && b.parse::<f64>().is_ok(),
        _ => false,
    }
}

/// Parse a console log (stdout + stderr).
pub fn parse_log(text: &str) -> LogInfo {
    parse_log_ignoring(text, &[])
}

/// [`parse_log`] that skips every line containing one of `ignore` (the scratch directory and the raw-file path the
/// harness itself passed to ngspice, which ngspice echoes).
pub fn parse_log_ignoring(text: &str, ignore: &[&str]) -> LogInfo {
    let mut info = LogInfo::default();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Total analysis time (seconds) =") {
            if let Ok(v) = rest.trim().parse::<f64>() {
                info.analysis_s = Some(v);
            }
        } else if let Some(rest) = t.strip_prefix("Total elapsed time (seconds) =") {
            if let Ok(v) = rest.trim().parse::<f64>() {
                info.elapsed_s = Some(v);
            }
        } else if t.starts_with("Using KLU") {
            info.solver = Some(NgSolver::Klu);
        } else if t.starts_with("Using SPARSE") {
            info.solver = Some(NgSolver::Sparse);
        }
        let low = t.to_ascii_lowercase();
        if low.starts_with("circuit:") || low.starts_with("title:") {
            continue; // the deck title may contain any word
        }
        if low.contains("raw file")
            && (low.starts_with("binary raw file") || low.starts_with("ascii raw file"))
        {
            continue; // `binary raw file "<scratch>/<case id>/ng_sparse.raw"`: an echo of the path we passed
        }
        if ignore.iter().any(|x| !x.is_empty() && t.contains(x)) || is_table_row(t) {
            continue;
        }
        if low.starts_with("warning") {
            info.warnings.push(t.to_string());
        } else if is_failure_line(&low) {
            info.failures.push(t.to_string());
        }
    }
    info
}

#[derive(Clone, Debug, PartialEq)]
pub enum NgClass {
    Ok,
    /// Non-zero exit, no rawfile, or a failure pattern in the log.
    Error(String),
    Timeout,
    NotInstalled,
}

#[derive(Clone, Debug)]
pub struct NgRun {
    pub child: ChildRun,
    pub log: LogInfo,
    pub class: NgClass,
    pub raw_path: PathBuf,
    /// Plots, if requested and the file could be read; otherwise the reason.
    pub plots: Result<Vec<Plot>, String>,
    /// Process starts used (2 when a crash was retried once).
    pub attempts: u32,
    /// Failure-pattern log lines that were deliberately not treated as a failure (`ignore_log_failures`).
    pub ignored_failures: Vec<String>,
    /// Complex plots (`.ac`, `.pz`, `.disto`) ngspice wrote that the suite does not compare.
    pub skipped_plots: Vec<String>,
    /// Set when the captured console output is incomplete in a way that matters (the timing lines are missing from
    /// a cut output, or a descendant kept the pipes open): the case report must say so.
    pub log_note: Option<String>,
}

/// Options of [`run_opts`]; `RunOpts::default()` is what [`run`] uses.
#[derive(Clone, Debug, Default)]
pub struct RunOpts<'a> {
    /// Working directory of the child (relative `.include`/`.lib` targets resolve against it).
    pub cwd: Option<&'a Path>,
    /// Do not let failure-pattern log lines (`singular matrix`, `Error`, ...) turn a run that exited 0 and wrote
    /// a rawfile into a failure (deck tag `ngspice_error_ok`).
    pub ignore_log_failures: bool,
    /// Start the process once more when it was killed by a signal (a SIGSEGV on some decks is not deterministic).
    pub retry_on_signal: bool,
}

/// Run `ngspice -b -r <raw> <deck>`. `expect_solver` is checked against the log (`Using KLU` / `Using SPARSE`).
pub fn run(
    ngspice: &str,
    deck: &Path,
    raw: &Path,
    lim: &Limits,
    expect_solver: Option<NgSolver>,
    want_plots: bool,
) -> NgRun {
    run_opts(
        ngspice,
        deck,
        raw,
        lim,
        expect_solver,
        want_plots,
        &RunOpts::default(),
    )
}

/// [`run`] with a working directory, tolerated log patterns and the crash retry.
pub fn run_opts(
    ngspice: &str,
    deck: &Path,
    raw: &Path,
    lim: &Limits,
    expect_solver: Option<NgSolver>,
    want_plots: bool,
    opts: &RunOpts,
) -> NgRun {
    let mut r = run_once(ngspice, deck, raw, lim, expect_solver, want_plots, opts);
    if opts.retry_on_signal && matches!(r.child.exit, Exit::Signal(_)) {
        let first = r.child.exit.clone();
        r = run_once(ngspice, deck, raw, lim, expect_solver, want_plots, opts);
        r.attempts = 2;
        if let NgClass::Error(m) = &mut r.class {
            m.push_str(&format!(
                " (retried once after the first attempt ended with {first:?})"
            ));
        }
    }
    r
}

fn run_once(
    ngspice: &str,
    deck: &Path,
    raw: &Path,
    lim: &Limits,
    expect_solver: Option<NgSolver>,
    want_plots: bool,
    opts: &RunOpts,
) -> NgRun {
    let _ = std::fs::remove_file(raw);
    let mut cmd = Command::new(ngspice);
    // `-n` (last, so the positions of the other arguments are unchanged): no `spinit` / `~/.spiceinit`, whose
    // commands (`set filetype=ascii`, `echo`, ...) would change the raw format, the timing and the log. The
    // initialisation files are looked up through HOME and `SPICE_*` variables: neutralised as well.
    cmd.arg("-b").arg("-r").arg(raw).arg(deck).arg("-n");
    for (k, _) in std::env::vars_os() {
        if k.to_string_lossy().starts_with("SPICE_") {
            cmd.env_remove(&k);
        }
    }
    if let Some(home) = raw.parent().filter(|p| p.is_dir()) {
        cmd.env("HOME", home);
    }
    if let Some(d) = opts.cwd {
        cmd.current_dir(d);
    }
    let child = match run_guarded(&mut cmd, lim) {
        Ok(c) => c,
        Err(e) => {
            let (class, msg) = match e {
                SpawnError::NotFound(m) => (NgClass::NotInstalled, m),
                SpawnError::Other(m) => (
                    NgClass::Error(format!("spawn failed: {m}")),
                    "spawn failed".to_string(),
                ),
            };
            return NgRun {
                child: ChildRun {
                    exit: Exit::Code(-1),
                    wall: Duration::ZERO,
                    stdout: String::new(),
                    stderr: msg.clone(),
                    output_truncated: false,
                    max_rss_kib: None,
                    rss_samples: 0,
                    cpu_us: None,
                    output_dropped_bytes: 0,
                    capture_incomplete: false,
                    rss_scope: "process",
                },
                log: LogInfo::default(),
                class,
                raw_path: raw.to_path_buf(),
                plots: Err(msg),
                attempts: 1,
                ignored_failures: Vec::new(),
                skipped_plots: Vec::new(),
                log_note: None,
            };
        }
    };
    // the console echoes the raw-file path and the scratch directory: they are ours, never a failure message
    let raw_s = raw.display().to_string();
    let dir_s = raw.parent().map(|p| p.display().to_string());
    let mut ignore: Vec<&str> = vec![raw_s.as_str()];
    if let Some(d) = dir_s.as_deref() {
        ignore.push(d);
    }
    let log = parse_log_ignoring(&format!("{}\n{}", child.stdout, child.stderr), &ignore);
    let mut class = match &child.exit {
        Exit::Timeout => NgClass::Timeout,
        Exit::RssCap => NgClass::Error("killed at the RSS cap".to_string()),
        Exit::Signal(s) => NgClass::Error(format!("terminated by signal {s}")),
        Exit::Code(0) => NgClass::Ok,
        Exit::Code(c) => NgClass::Error(format!("exit code {c}")),
    };
    let mut ignored_failures = Vec::new();
    if class == NgClass::Ok {
        if opts.ignore_log_failures {
            ignored_failures = log.failures.clone();
        }
        if let (Some(f), false) = (log.failures.first(), opts.ignore_log_failures) {
            class = NgClass::Error(format!("log: {f}"));
        } else if !raw.exists() {
            class = NgClass::Error("no rawfile written".to_string());
        } else if let (Some(want), Some(got)) = (expect_solver, log.solver) {
            if want != got {
                class = NgClass::Error(format!(
                    "solver mismatch: asked for {}, log says {}",
                    want.as_str(),
                    got.as_str()
                ));
            }
        }
    }
    let mut skipped_plots = Vec::new();
    let mut plots = if want_plots && class == NgClass::Ok {
        rawfile::read_file_lenient(raw)
            .map(|l| {
                skipped_plots = l.skipped;
                l.plots
            })
            .map_err(|e| e.to_string())
    } else if want_plots {
        // A failed run may still have left a partial rawfile; keep it unread (artifacts only).
        Err("not loaded (run failed)".to_string())
    } else {
        Err("not requested".to_string())
    };
    if let (NgClass::Ok, Err(e)) = (&class, &plots) {
        if want_plots {
            class = NgClass::Error(format!("rawfile unreadable: {e}"));
        }
    }
    if let (NgClass::Ok, Ok(p)) = (&class, &plots) {
        // a header-only rawfile (`.dc` with a wrong-sign step) means ngspice produced no data at all: the run
        // failed, so its plots are not offered to the comparison either
        if !p.is_empty() && p.iter().all(|x| x.npoints == 0) {
            class = NgClass::Error("rawfile has no data rows".to_string());
            plots = Err("not loaded (rawfile has no data rows)".to_string());
        }
    }
    let timing_lines_missing = log.analysis_s.is_none() || log.elapsed_s.is_none();
    let log_note = if child.capture_incomplete {
        Some(
            "a process started by ngspice kept the output pipes open: the console log is incomplete (what was read by the end of the grace period is used)"
                .to_string(),
        )
    } else if child.output_dropped_bytes > 0 && class == NgClass::Ok && timing_lines_missing {
        Some(format!(
            "the console output was cut in the middle ({} bytes omitted, head and tail kept) and `Total analysis time` / `Total elapsed time` are not in what was kept",
            child.output_dropped_bytes
        ))
    } else {
        None
    };
    NgRun {
        child,
        log,
        class,
        raw_path: raw.to_path_buf(),
        plots,
        attempts: 1,
        ignored_failures,
        skipped_plots,
        log_note,
    }
}

/// `ngspice --version` -> `ngspice-47` style string; the reason when this is not a usable ngspice. A usable one
/// starts, exits 0 within 10 s and its output names itself `ngspice`. (`--ngspice /bin/false` used to count as
/// installed: the version probe only had to SPAWN, so every case ended `ngspice_error`, `loud_error` decks passed and
/// the run was a green PASS.)
pub fn version_probe(ngspice: &str) -> Result<String, String> {
    let mut cmd = Command::new(ngspice);
    cmd.arg("--version");
    let lim = Limits {
        timeout: Duration::from_secs(10),
        max_rss_kib: None,
        pin_cpu: None,
    };
    let r = run_guarded(&mut cmd, &lim).map_err(|e| match e {
        SpawnError::NotFound(m) => format!("`{ngspice}` was not found: {m}"),
        SpawnError::Other(m) => format!("`{ngspice}` cannot be started: {m}"),
    })?;
    match &r.exit {
        Exit::Code(0) => {}
        Exit::Timeout => {
            return Err(format!("`{ngspice} --version` did not finish within 10 s"));
        }
        other => {
            return Err(format!(
                "`{ngspice} --version` did not exit cleanly ({other:?}): {}",
                r.stderr.lines().next().unwrap_or("no message").trim()
            ));
        }
    }
    let text = format!("{}\n{}", r.stdout, r.stderr);
    for tok in text.split_whitespace() {
        if let Some(v) = tok.strip_prefix("ngspice-") {
            if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return Ok(format!(
                    "ngspice-{}",
                    v.trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.')
                ));
            }
        }
    }
    match text
        .lines()
        .find(|l| l.to_ascii_lowercase().contains("ngspice"))
    {
        Some(l) => Ok(l.trim().trim_matches('*').trim().to_string()),
        None => Err(format!(
            "`{ngspice} --version` exited 0 but its output does not mention ngspice: `{}`",
            text.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("(empty)")
                .trim()
        )),
    }
}

/// [`version_probe`] without the reason: `None` when ngspice is missing or not usable.
pub fn version(ngspice: &str) -> Option<String> {
    version_probe(ngspice).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_batch_log() {
        let log = "Using SPARSE 1.3 as Direct Linear Solver\nNo. of Data Rows : 70\nTotal analysis time (seconds) = 0.000414451\n\nTotal elapsed time (seconds) = 0.002 \n";
        let i = parse_log(log);
        assert_eq!(i.analysis_s, Some(0.000414451));
        assert_eq!(i.elapsed_s, Some(0.002));
        assert_eq!(i.solver, Some(NgSolver::Sparse));
        assert!(i.failures.is_empty());
        // small values are printed with an upper-case exponent
        let i = parse_log(
            "Total analysis time (seconds) = 9.618E-05\nTotal elapsed time (seconds) = 0 \n",
        );
        assert_eq!(i.analysis_s, Some(9.618e-5));
        assert_eq!(i.elapsed_s, Some(0.0));
        let i = parse_log("Using KLU as Direct Linear Solver\n");
        assert_eq!(i.solver, Some(NgSolver::Klu));
    }

    #[test]
    fn detects_failure_patterns_but_not_warnings() {
        let log = "Warning: singular matrix: check node x\ndoAnalyses: TRAN:  Timestep too small; time = 1e-9\nError: no such device\nsimulation(s) aborted\nNo convergence in DC\nToo many iterations\n";
        let i = parse_log(log);
        assert_eq!(i.warnings.len(), 1);
        assert_eq!(i.failures.len(), 5, "{:?}", i.failures);
        assert!(parse_log("Circuit: error in my title\n")
            .failures
            .is_empty());
    }

    #[test]
    fn missing_ngspice_is_not_installed() {
        let lim = Limits {
            timeout: Duration::from_secs(5),
            max_rss_kib: None,
            pin_cpu: None,
        };
        let r = run(
            "/nonexistent/ngspice",
            Path::new("/dev/null"),
            Path::new("/tmp/x.raw"),
            &lim,
            None,
            false,
        );
        assert_eq!(r.class, NgClass::NotInstalled);
        assert!(version("/nonexistent/ngspice").is_none());
    }

    // ---- fake ngspice executables: the retry, cwd and log-pattern semantics are testable without ngspice ------

    use std::os::unix::fs::PermissionsExt;

    use super::super::rustsim::TmpDir;

    const RAW_OK: &str = "printf 'Title: t\\nPlotname: Operating Point\\nFlags: real\\nNo. Variables: 2\\nNo. Points: 1\\nVariables:\\n\\t0\\tv(a)\\tvoltage\\n\\t1\\tv1#branch\\tcurrent\\nValues:\\n 0\\t1.5\\n\\t-1e-3\\n' > \"$RAW\"";

    /// `#!/bin/sh` script `name` in `dir` that sees `$RAW` (the `-r` argument), `$DECK` and the working directory.
    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let text = format!("#!/bin/sh\nulimit -c 0\nRAW=\"$3\"\nDECK=\"$4\"\n{body}\n");
        std::fs::write(&p, text).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn lim() -> Limits {
        Limits {
            timeout: Duration::from_secs(20),
            max_rss_kib: None,
            pin_cpu: None,
        }
    }

    fn go(exe: &Path, dir: &Path, opts: &RunOpts) -> NgRun {
        // a freshly written script can be briefly busy when another test forks at the same moment
        for _ in 0..20 {
            let r = run_opts(
                exe.to_str().unwrap(),
                &dir.join("deck.sp"),
                &dir.join("out.raw"),
                &lim(),
                None,
                true,
                opts,
            );
            if !matches!(&r.class, NgClass::Error(m) if m.contains("busy")) {
                return r;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("fake ngspice stayed busy");
    }

    #[test]
    fn a_crash_is_retried_once_and_a_deterministic_crash_is_an_error() {
        let t = TmpDir::new("ng-retry").unwrap();
        let d = t.path();
        // crashes on the first start only
        let flaky = fake(
            d,
            "flaky",
            &format!(
                "if [ ! -e \"$0.seen\" ]; then : > \"$0.seen\"; kill -SEGV $$; fi\necho 'Using SPARSE 1.3 as Direct Linear Solver'\n{RAW_OK}"
            ),
        );
        let retry = RunOpts {
            retry_on_signal: true,
            ..Default::default()
        };
        let r = go(&flaky, d, &retry);
        assert_eq!(r.class, NgClass::Ok, "{r:?}");
        assert_eq!(r.attempts, 2);
        assert_eq!(r.plots.as_ref().unwrap()[0].col("v(a)").unwrap(), &[1.5]);
        // without the retry the first crash stands
        let _ = std::fs::remove_file(d.join("flaky.seen"));
        let r = go(&flaky, d, &RunOpts::default());
        assert!(
            matches!(&r.class, NgClass::Error(m) if m.contains("signal 11")),
            "{r:?}"
        );
        assert_eq!(r.attempts, 1);
        // a deterministic crash stays an error after the single retry
        let always = fake(d, "always", "kill -SEGV $$");
        let r = go(&always, d, &retry);
        assert_eq!(r.attempts, 2);
        assert!(
            matches!(&r.class, NgClass::Error(m) if m.contains("retried once")),
            "{r:?}"
        );
        // ordinary failures (exit code, timeout) are never retried
        let bad = fake(d, "bad", "exit 1");
        let r = go(&bad, d, &retry);
        assert_eq!(r.attempts, 1);
        assert!(matches!(&r.class, NgClass::Error(m) if m.contains("exit code 1")));
    }

    #[test]
    fn log_failure_patterns_are_tolerated_only_on_request() {
        let t = TmpDir::new("ng-logok").unwrap();
        let d = t.path();
        let noisy = fake(
            d,
            "noisy",
            &format!("echo 'Warning: singular matrix: check node 4'\necho 'doAnalyses: Error: no convergence'\n{RAW_OK}"),
        );
        let r = go(&noisy, d, &RunOpts::default());
        assert!(
            matches!(&r.class, NgClass::Error(m) if m.starts_with("log:")),
            "{r:?}"
        );
        let ok = RunOpts {
            ignore_log_failures: true,
            ..Default::default()
        };
        let r = go(&noisy, d, &ok);
        assert_eq!(r.class, NgClass::Ok, "{r:?}");
        assert_eq!(r.ignored_failures.len(), 1);
        // the tolerance never hides a missing rawfile or a non-zero exit
        let norw = fake(d, "norw", "echo 'singular matrix'");
        assert!(matches!(&go(&norw, d, &ok).class, NgClass::Error(m) if m.contains("no rawfile")));
        let fail = fake(
            d,
            "fail",
            &format!("echo 'singular matrix'\n{RAW_OK}\nexit 2"),
        );
        assert!(matches!(&go(&fail, d, &ok).class, NgClass::Error(m) if m.contains("exit code 2")));
    }

    #[test]
    fn the_child_runs_in_the_requested_directory() {
        let t = TmpDir::new("ng-cwd").unwrap();
        let d = t.path();
        let inc = d.join("incdir");
        std::fs::create_dir_all(&inc).unwrap();
        std::fs::write(inc.join("models.lib"), ".model D1 D\n").unwrap();
        // succeeds only when a relative .include target is readable from the working directory
        let s = fake(
            d,
            "needs_inc",
            &format!("cat models.lib > /dev/null || exit 3\n{RAW_OK}"),
        );
        let r = go(&s, d, &RunOpts::default());
        assert!(
            matches!(&r.class, NgClass::Error(m) if m.contains("exit code 3")),
            "{r:?}"
        );
        let with = RunOpts {
            cwd: Some(&inc),
            ..Default::default()
        };
        assert_eq!(go(&s, d, &with).class, NgClass::Ok);
    }

    /// ER-process-01: the raw-file path (with the case id in it) and the `.tran` node table are echoes, not failures.
    #[test]
    fn echoed_paths_and_node_table_rows_are_not_failure_messages() {
        let log = "binary raw file \"/dev/shm/rustspice-bench-1-suite/tran_idonly_error/ng_sparse.raw\"\nInitial Transient Solution\n--------------------------\n\nNode                                   Voltage\n----                                   -------\nerror_amp                                    0\nfatal_node                              1.5e-3\nv1#branch                                -0.001\nTotal analysis time (seconds) = 0.01\n";
        let i = parse_log(log);
        assert!(i.failures.is_empty(), "{:?}", i.failures);
        assert_eq!(i.analysis_s, Some(0.01));
        // the scratch path the harness itself passed is ignored wherever it appears
        let i = parse_log_ignoring(
            "ngspice: cannot open /dev/shm/rustspice-bench-9-suite/error_x/ng.raw: Error 5\n",
            &["/dev/shm/rustspice-bench-9-suite/error_x"],
        );
        assert!(i.failures.is_empty(), "{:?}", i.failures);
        // real failure messages still count, with or without a `word:` prefix
        for line in [
            "Error: no such device\n",
            "ERROR: unknown subckt: x1\n",
            "doAnalyses: TRAN:  Timestep too small; time = 1e-9\n",
            "run simulation(s) aborted\n",
            "No convergence in DC operation\n",
            "Too many iterations without convergence\n",
            "singular matrix:  check node 4\n",
            "ngspice-47: fatal error: out of memory\n",
            "Fatal: nope\n",
        ] {
            assert_eq!(parse_log(line).failures.len(), 1, "{line}");
        }
        // a message that only CONTAINS a failure word mid-sentence is not one
        for line in [
            "Reference value =  1.0e-3 (no error)\n",
            "Note: the error handling of the deck is disabled\n",
            "error_count = 0\n",
        ] {
            assert!(parse_log(line).failures.is_empty(), "{line}");
        }
    }

    #[test]
    fn a_case_id_with_error_in_it_does_not_fail_a_good_run() {
        let t = TmpDir::new("ng-echo").unwrap();
        // the directory name plays the case id of the suite's scratch layout
        let d = t.path().join("tran_idonly_error");
        std::fs::create_dir_all(&d).unwrap();
        let s = fake(
            &d,
            "echoer",
            &format!("echo \"binary raw file \\\"$RAW\\\"\"\n{RAW_OK}"),
        );
        let r = go(&s, &d, &RunOpts::default());
        assert!(r.child.stdout.contains("binary raw file"), "{r:?}");
        assert_eq!(r.class, NgClass::Ok, "{r:?}");
        assert!(r.log.failures.is_empty());
    }

    #[test]
    fn the_command_line_has_no_init_file_and_the_environment_is_neutral() {
        let t = TmpDir::new("ng-args").unwrap();
        let d = t.path();
        let s = fake(
            d,
            "echo_args",
            &format!("echo \"ARGS=$*\"\necho \"HOME=$HOME\"\n{RAW_OK}"),
        );
        let r = go(&s, d, &RunOpts::default());
        assert_eq!(r.class, NgClass::Ok, "{r:?}");
        // `-n` goes last: the first four positions (`-b -r <raw> <deck>`) are what fake scripts and logs rely on
        let args = r
            .child
            .stdout
            .lines()
            .find_map(|l| l.strip_prefix("ARGS="))
            .unwrap();
        let toks: Vec<&str> = args.split_whitespace().collect();
        assert_eq!(toks[0], "-b");
        assert_eq!(toks[1], "-r");
        assert!(toks[2].ends_with("out.raw") && toks[3].ends_with("deck.sp"));
        assert_eq!(toks[4], "-n");
        // HOME is the scratch directory of the run, never the user's home (`~/.spiceinit`)
        assert!(
            r.child
                .stdout
                .lines()
                .any(|l| l == format!("HOME={}", d.display())),
            "{}",
            r.child.stdout
        );
    }

    /// ER-process-02: the timing lines are printed at the END, after a node table that can exceed the capture.
    #[test]
    fn the_timing_lines_survive_a_large_console_output() {
        let t = TmpDir::new("ng-bigout").unwrap();
        let d = t.path();
        let s = fake(
            d,
            "big",
            &format!(
                "echo 'Using SPARSE 1.3 as Direct Linear Solver'\ni=0\nwhile [ $i -lt 60000 ]; do echo \"n$i   0.5\"; i=$((i+1)); done\necho 'Total analysis time (seconds) = 0.25'\necho 'Total elapsed time (seconds) = 0.5'\n{RAW_OK}"
            ),
        );
        let r = go(&s, d, &RunOpts::default());
        assert!(
            r.child.output_dropped_bytes > 0,
            "the output must exceed the capture: {r:?}"
        );
        assert_eq!(r.class, NgClass::Ok, "{r:?}");
        assert_eq!(r.log.analysis_s, Some(0.25));
        assert_eq!(r.log.elapsed_s, Some(0.5));
        assert_eq!(r.log.solver, Some(NgSolver::Sparse));
        assert!(r.log_note.is_none(), "{:?}", r.log_note);
        // when the timing lines really are missing from a cut output, the case says so
        let s = fake(
            d,
            "big_no_timing",
            &format!(
                "i=0\nwhile [ $i -lt 60000 ]; do echo \"n$i   0.5\"; i=$((i+1)); done\n{RAW_OK}"
            ),
        );
        let r = go(&s, d, &RunOpts::default());
        assert!(r.log.analysis_s.is_none());
        assert!(
            r.log_note
                .as_deref()
                .is_some_and(|n| n.contains("cut in the middle")),
            "{:?}",
            r.log_note
        );
    }

    /// ER-driver-02: a `--version` that merely spawns is not an ngspice.
    #[test]
    fn a_fake_or_broken_ngspice_is_not_usable() {
        let t = TmpDir::new("ng-version").unwrap();
        let d = t.path();
        let mk = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            // a script that was just written can be briefly busy while another test forks
            std::thread::sleep(Duration::from_millis(30));
            p
        };
        // exits non-zero
        let e = version_probe("/bin/false").unwrap_err();
        assert!(e.contains("did not exit cleanly"), "{e}");
        // exits 0 but is something else
        let other = mk("other", "echo 'false (GNU coreutils) 9.12'");
        let e = version_probe(other.to_str().unwrap()).unwrap_err();
        assert!(e.contains("does not mention ngspice"), "{e}");
        // a crash
        let crash = mk("crash", "kill -SEGV $$");
        assert!(version_probe(crash.to_str().unwrap()).is_err());
        // missing
        let e = version_probe("/nonexistent/ngspice").unwrap_err();
        assert!(e.contains("not found"), "{e}");
        // the real thing, and a fake that identifies itself
        let ok = mk(
            "okng",
            "echo '** ngspice-47 : Circuit level simulation program'",
        );
        assert_eq!(version_probe(ok.to_str().unwrap()).unwrap(), "ngspice-47");
        let named = mk("named", "echo 'My ngspice build'");
        assert!(version_probe(named.to_str().unwrap())
            .unwrap()
            .contains("ngspice"));
        assert!(version("/bin/false").is_none());
    }

    #[test]
    fn header_only_rawfile_is_no_data() {
        let t = TmpDir::new("ng-nodata").unwrap();
        let d = t.path();
        let s = fake(
            d,
            "hdr",
            "printf 'Title: t\\nPlotname: DC transfer characteristic\\nFlags: real\\nNo. Variables: 2\\nNo. Points: 0\\nVariables:\\n\\t0\\tv-sweep\\tvoltage\\n\\t1\\tv(a)\\tvoltage\\nBinary:\\n' > \"$RAW\"",
        );
        let r = go(&s, d, &RunOpts::default());
        assert!(
            matches!(&r.class, NgClass::Error(m) if m.contains("no data rows")),
            "{r:?}"
        );
        // the empty plots are not offered to the comparison
        assert!(r.plots.is_err(), "{r:?}");
    }
}
