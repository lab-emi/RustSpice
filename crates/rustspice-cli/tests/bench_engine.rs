//! End-to-end tests of the benchmark engine: a tiny corpus is built in a temp dir and the REAL binary runs it against
//! the real ngspice. Tests that need ngspice skip (printing why) when it is not installed; the "ngspice missing"
//! semantics are tested separately with a bogus executable path.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_rustspice");

fn ngspice_available() -> bool {
    Command::new("ngspice")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn scratch(name: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("bench_engine_{name}"));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn put(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

const RC_OP: &str = "RC divider operating point
* bench.id: rc_op
* bench.group: accuracy
* bench.title: RC divider DC operating point
* bench.probes: v(in), v(out), i(v1)
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
C1 out 0 1n
.op
.end
";

const DIODE_OP: &str = "Diode resistor operating point
* bench.id: diode_op
* bench.group: accuracy
* bench.title: Diode forward bias
* bench.tol.rel: 1e-6
V1 in 0 DC 5
R1 in d 1k
D1 d 0 DM
.model DM D(IS=1e-14 N=1)
.op
.end
";

const REFUSE: &str = "Malformed resistor must be refused
* bench.id: refuse_malformed
* bench.group: robustness
* bench.expect: loud_error
V1 a 0 DC 1
R2 a b
.op
.end
";

const CLAMP_STACK: &str = "Two stacked 5 V sources feeding R + VCVS
* bench.id: clamp_rail
* bench.group: capability
* bench.known_gap: clamp-rail
* bench.gap.note: Newton clamps node voltages to the DC source range
V1 a 0 DC 5
V2 b a DC 5
E1 c 0 b 0 1.0
R1 c 0 1k
.op
.end
";

const GAP_CLOSED: &str = "Passing deck wrongly marked as a known gap
* bench.id: gap_now_closed
* bench.group: capability
* bench.known_gap: clamp-rail
* bench.gap.note: the stale gap of a deck that now passes
V1 a 0 DC 5
R1 a b 1k
R2 b 0 1k
.op
.end
";

const BAD_META: &str = "Deck with invalid metadata
* bench.id: bad_meta
* bench.group: capability
* bench.expect: maybe
V1 a 0 DC 1
R1 a 0 1k
.op
.end
";

const PERF_LADDER: &str = "Small resistor ladder
* bench.id: perf_ladder
* bench.group: performance
* bench.size: 51
* bench.tol.rel: 1e-6
V1 n0 0 DC 1
R1 n0 n1 100
R2 n1 0 1k
R3 n1 n2 100
R4 n2 0 1k
R5 n2 n3 100
R6 n3 0 1k
.op
.end
";

const RC_SERIES: &str = "RC step work-precision series
* bench.id: rc_series
* bench.group: accuracy
* bench.expect: either
* bench.probes: v(out)
* bench.series.tstep: 100n 50n 25n
V1 in 0 PULSE(0 1 0 10n 10n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 100n 5u
.end
";

fn run_suite(cases: &Path, extra: &[&str]) -> (Output, PathBuf, PathBuf, PathBuf) {
    let json = cases.join("out.json");
    let md = cases.join("out.md");
    let art = cases.join("artifacts");
    let out = Command::new(BIN)
        .arg("bench-suite")
        .arg("--cases")
        .arg(cases)
        .args([
            "--no-fuzz",
            "--family",
            "static",
            "--scale",
            "small",
            "--case-timeout",
            "30",
            "--max-rss-mib",
            "2048",
        ])
        .arg("--json")
        .arg(&json)
        .arg("--report")
        .arg(&md)
        .arg("--artifacts")
        .arg(&art)
        .args(extra)
        .output()
        .unwrap();
    (out, json, md, art)
}

fn case<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("case {id} missing"))
}

#[test]
fn end_to_end_against_real_ngspice() {
    if !ngspice_available() {
        eprintln!("SKIP end_to_end_against_real_ngspice: ngspice not installed (suite would report `unverified`)");
        return;
    }
    let dir = scratch("e2e");
    // the cases directory is only the corpus; outputs go next to it
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    for (f, t) in [
        ("rc_op.sp", RC_OP),
        ("diode_op.sp", DIODE_OP),
        ("refuse_malformed.sp", REFUSE),
        ("clamp_rail.sp", CLAMP_STACK),
        ("gap_now_closed.sp", GAP_CLOSED),
        ("bad_meta.sp", BAD_META),
        ("perf_ladder.sp", PERF_LADDER),
        ("rc_series.sp", RC_SERIES),
    ] {
        put(&cases, f, t);
    }
    let json = dir.join("out.json");
    let md = dir.join("out.md");
    let art = dir.join("artifacts");
    let out = Command::new(BIN)
        .arg("bench-suite")
        .arg("--cases")
        .arg(&cases)
        .args([
            "--no-fuzz",
            "--family",
            "static",
            "--scale",
            "small",
            "--reps",
            "5",
            "--case-timeout",
            "30",
            "--max-rss-mib",
            "2048",
        ])
        .arg("--json")
        .arg(&json)
        .arg("--report")
        .arg(&md)
        .arg("--artifacts")
        .arg(&art)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr:\n{stderr}");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(v["schema"], 1);
    // ER-driver-14: the binary's own source tree and the corpus are reported apart (the old report took the git
    // state of the `--cases` directory for the version of the binary)
    assert_eq!(v["rustspice"]["source_dir"], env!("CARGO_MANIFEST_DIR"));
    assert_eq!(v["corpus"]["path"], cases.display().to_string());
    assert!(v["slower_than_ngspice"].is_array());
    let st = |id: &str| case(&v, id)["status"].as_str().unwrap().to_string();

    assert_eq!(st("rc_op"), "pass", "{:#?}", case(&v, "rc_op"));
    assert_eq!(st("diode_op"), "pass", "{:#?}", case(&v, "diode_op"));
    assert_eq!(st("refuse_malformed"), "pass");
    assert_eq!(case(&v, "refuse_malformed")["outcome"], "loud_error");
    // known gap: RustSpice currently cannot solve this (Newton clamps to the source range)
    assert_eq!(
        st("clamp_rail"),
        "known_gap_confirmed",
        "{:#?}",
        case(&v, "clamp_rail")
    );
    assert_eq!(case(&v, "clamp_rail")["known_gap"], "clamp-rail");
    assert_eq!(case(&v, "clamp_rail")["outcome"], "loud_error");
    // a known gap that no longer fails is reported as closed
    assert_eq!(st("gap_now_closed"), "gap_closed");
    // invalid metadata is reported, never dropped
    assert_eq!(st("bad_meta"), "unverified");
    assert!(case(&v, "bad_meta")["notes"]
        .to_string()
        .contains("match|loud_error|either"));
    assert_eq!(v["summary"]["verdict"], "PASS");
    assert_eq!(v["summary"]["total"], 10); // 7 plain decks + 3 series members

    // op accuracy numbers are real and explainable: v(out) = 5 * 3/4 = 3.75, i(v1) = -1.25 mA
    let probes = case(&v, "rc_op")["probes"].as_array().unwrap();
    assert_eq!(probes.len(), 3);
    for p in probes {
        assert!(p["abs_err"].as_f64().unwrap() < 1e-9, "{p}");
        assert_eq!(p["vacuous"], false);
    }
    assert_eq!(case(&v, "rc_op")["ref_status"], "tight");

    // artifacts of the non-passing known-gap case
    assert!(art.join("clamp_rail").join("deck.sp").exists());
    assert!(art.join("clamp_rail").join("rs.stderr").exists());
    assert!(!art.join("rc_op").exists());

    // timing of the performance case: 5 interleaved reps, three contenders, no zeros
    let perf = case(&v, "perf_ladder");
    assert_eq!(perf["timing_status"], "measured", "{perf:#?}");
    let t = &perf["timing"];
    assert_eq!(t["reps_done"], 5);
    assert_eq!(t["contenders"].as_array().unwrap().len(), 3);
    for c in t["contenders"].as_array().unwrap() {
        assert_eq!(c["e2e_us"]["n"], 5, "{c}");
        assert!(
            c["e2e_us"]["min"].as_f64().unwrap() > 100.0,
            "process wall below 100 us is implausible: {c}"
        );
    }
    let order: Vec<&str> = t["order_trace"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(order.len(), 5);
    assert_ne!(
        order[0], order[1],
        "execution order must rotate between repetitions"
    );
    assert!(t["headline"]["e2e_median"].as_f64().unwrap() > 0.0);
    // with two contenders the order strictly alternates A-first / B-first
    let two: Vec<&str> = case(&v, "rc_series__s1")["timing"]["order_trace"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(two.len(), 5);
    for w in two.windows(2) {
        assert_ne!(w[0], w[1]);
    }
    assert_eq!(two[0], two[2]);
    assert!(t["headline"]["solve_median"].as_f64().is_some());

    // work-precision series
    let series = v["series"].as_array().unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0]["base"], "rc_series");
    assert_eq!(series[0]["points"].as_array().unwrap().len(), 3);

    // Markdown: nine sections, known gaps grouped by slug
    let text = std::fs::read_to_string(&md).unwrap();
    for h in [
        "## 1. Verdict",
        "## 2. Known gaps",
        "## 3. Capability matrix",
        "## 4. Accuracy",
        "## 5. Robustness",
        "## 6. Performance",
        "## 7. Scaling fits",
        "## 8. Methodology",
        "## 9. Reproduction",
    ] {
        assert!(text.contains(h), "missing `{h}` in report");
    }
    assert!(text.contains("`clamp-rail`"));
    assert!(text.contains("Known gaps now CLOSED"));
    assert!(text.contains("gap_now_closed"));
}

#[test]
fn failing_case_without_known_gap_fails_the_run() {
    if !ngspice_available() {
        eprintln!("SKIP failing_case_without_known_gap_fails_the_run: ngspice not installed");
        return;
    }
    let cases = scratch("fail");
    put(
        &cases,
        "clamp_nogap.sp",
        &CLAMP_STACK
            .replace("clamp_rail", "clamp_nogap")
            .replace("* bench.known_gap: clamp-rail\n", "")
            .replace(
                "* bench.gap.note: Newton clamps node voltages to the DC source range\n",
                "",
            ),
    );
    put(&cases, "rc_op.sp", RC_OP);
    let (out, json, _md, art) = run_suite(&cases, &["--reps", "5"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(case(&v, "clamp_nogap")["status"], "fail_robustness");
    assert_eq!(case(&v, "rc_op")["status"], "pass");
    assert_eq!(v["summary"]["verdict"], "FAIL");
    assert!(art.join("clamp_nogap").join("repro.txt").exists());
}

#[test]
fn missing_ngspice_is_unverified_never_a_pass() {
    let cases = scratch("nong");
    put(&cases, "rc_op.sp", RC_OP);
    let (out, json, _md, _art) = run_suite(&cases, &["--ngspice", "/nonexistent/ngspice"]);
    assert_eq!(out.status.code(), Some(2));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(case(&v, "rc_op")["status"], "unverified");
    assert_eq!(v["summary"]["verdict"], "UNVERIFIED");
    assert_eq!(v["ngspice"]["installed"], false);
    let (out, _json, _md, _art) = run_suite(
        &cases,
        &["--ngspice", "/nonexistent/ngspice", "--allow-unverified"],
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn list_and_empty_corpus_exit_codes() {
    let cases = scratch("list");
    put(&cases, "rc_op.sp", RC_OP);
    let out = Command::new(BIN)
        .args([
            "bench-suite",
            "--no-fuzz",
            "--family",
            "static",
            "--scale",
            "small",
            "--list",
            "--cases",
        ])
        .arg(&cases)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("rc_op\taccuracy\t"), "{text}");
    assert_eq!(
        text.lines().count(),
        1,
        "--family static excludes generated decks: {text}"
    );
    let empty = scratch("empty");
    let out = Command::new(BIN)
        .args([
            "bench-suite",
            "--no-fuzz",
            "--family",
            "static",
            "--scale",
            "small",
            "--cases",
        ])
        .arg(&empty)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn run_writes_rawfile_and_stats_and_refuses_cleanly() {
    let dir = scratch("run");
    put(&dir, "ok.sp", RC_OP);
    let raw = dir.join("ok.raw");
    let stats = dir.join("ok.json");
    let out = Command::new(BIN)
        .arg("run")
        .arg(dir.join("ok.sp"))
        .arg("--raw")
        .arg(&raw)
        .arg("--stats")
        .arg(&stats)
        .output()
        .unwrap();
    assert!(out.status.success());
    // stdout behaviour unchanged
    assert!(String::from_utf8_lossy(&out.stdout).contains("[DC OP] Converged"));
    let bytes = std::fs::read(&raw).unwrap();
    let hdr_end = bytes.windows(8).position(|w| w == b"Binary:\n").unwrap() + 8;
    let hdr = String::from_utf8_lossy(&bytes[..hdr_end]);
    assert!(hdr.contains("Plotname: Operating Point") && hdr.contains("Flags: real"));
    assert!(
        hdr.contains("\t0\tv(in)\tvoltage") && hdr.contains("\t2\ti(v1)\tcurrent"),
        "{hdr}"
    );
    assert_eq!(bytes.len() - hdr_end, 3 * 8);
    // SPICE sign convention: the source delivers current, so i(v1) < 0 (ngspice convention)
    let vals: Vec<f64> = bytes[hdr_end..]
        .chunks(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert!(
        (vals[0] - 5.0).abs() < 1e-12
            && (vals[1] - 3.75).abs() < 1e-9
            && (vals[2] + 1.25e-3).abs() < 1e-12,
        "{vals:?}"
    );
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
    assert_eq!(s["status"], "ok");
    assert_eq!(s["analysis"], "op");
    assert_eq!(s["points"], 1);
    assert!(s["parse_us"].as_f64().unwrap() >= 0.0 && s["compile_us"].as_f64().unwrap() > 0.0);
    assert!(s["peak_rss_kib"].as_u64().unwrap() > 0);
    assert!(
        s["solver_compile_us"].is_null(),
        "unavailable numbers are null, not 0"
    );

    // a refused deck: non-zero exit, error on stderr, no rawfile, stats say so
    put(&dir, "bad.sp", "bad\nR1 a 0 1k\nR2 a b\n.op\n.end\n");
    let raw2 = dir.join("bad.raw");
    std::fs::write(&raw2, b"stale").unwrap();
    let stats2 = dir.join("bad.json");
    let out = Command::new(BIN)
        .arg("run")
        .arg(dir.join("bad.sp"))
        .arg("--raw")
        .arg(&raw2)
        .arg("--stats")
        .arg(&stats2)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Resistor requires"));
    assert!(
        !raw2.exists(),
        "a failed run must not leave a (stale or partial) rawfile"
    );
    let s2: Value = serde_json::from_str(&std::fs::read_to_string(&stats2).unwrap()).unwrap();
    assert!(s2["status"].as_str().unwrap().starts_with("error:"));
}

// ---------------------------------------------------------------------------------------------------------
// Phase 2 semantics (SUITE_SPEC 13.3), against the real ngspice
// ---------------------------------------------------------------------------------------------------------

fn run_one(name: &str, files: &[(&str, &[u8])]) -> Option<(Value, PathBuf)> {
    if !ngspice_available() {
        eprintln!("SKIP {name}: ngspice not installed");
        return None;
    }
    let dir = scratch(name);
    let cases = dir.join("cases");
    for (f, bytes) in files {
        let p = cases.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    let (out, json, _md, art) = run_suite(&cases, &[]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    Some((v, art))
}

#[test]
fn relative_include_resolves_against_the_deck_directory() {
    // ngspice must find the lib from a temp-deck run elsewhere: both tools run with cwd = the deck directory
    let deck = "Include a model file next to the deck
* bench.id: inc_rel
* bench.group: capability
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: include ignored
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DINC
.include inc_rel_models.lib
.op
.end
";
    let Some((v, _)) = run_one(
        "include",
        &[
            ("sub/inc_rel.sp", deck.as_bytes()),
            ("sub/inc_rel_models.lib", b".model DINC D(IS=1e-12 N=1.5)\n"),
        ],
    ) else {
        return;
    };
    let c = case(&v, "inc_rel");
    assert_eq!(c["ngspice_status"], "ok", "{c:#?}");
    assert_eq!(c["ref_status"], "tight", "{c:#?}");
    // RustSpice ignores .include (known gap): the model differs from ngspice's
    assert!(
        ["known_gap_confirmed", "gap_closed"].contains(&c["status"].as_str().unwrap()),
        "{c:#?}"
    );
}

fn tree_listing(root: &std::path::Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap().display().to_string();
            // the harness helper writes its own outputs next to the decks
            if rel.starts_with("out.") || rel.starts_with("artifacts") {
                continue;
            }
            let len = p.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(format!("{rel} {len}"));
            if p.is_dir() {
                walk(&p, root, out);
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v.sort();
    v
}

#[test]
fn the_corpus_directory_is_never_written_to() {
    // ngspice writes `b3v33check.log` into its working directory when a BSIM3 card is checked; the tools run in a
    // scratch directory that only links the deck's siblings, so the corpus tree is identical after the run
    let bsim = "NMOS LEVEL=49 card
* bench.id: nowrite_bsim
* bench.group: capability
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.known_gap: silent-wrong-model-level
* bench.gap.note: level 49 evaluated as another model
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=49 VERSION=3.3.0)
.dc VDS 0 3 0.5
.end
";
    let chain = "Nested include chain
* bench.id: nowrite_chain
* bench.group: capability
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: include ignored
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DCHAIN
.include nowrite_a.lib
.op
.end
";
    let dir = scratch("nowrite");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    for (f, b) in [
        ("nowrite_bsim.sp", bsim),
        ("nowrite_chain.sp", chain),
        ("nowrite_a.lib", ".include nowrite_b.lib\n"),
        ("nowrite_b.lib", ".model DCHAIN D(IS=1e-12 N=1.5)\n"),
        ("nowrite_unrelated.lib", ".model DX D(IS=1e-9)\n"),
    ] {
        std::fs::write(cases.join(f), b).unwrap();
    }
    if !ngspice_available() {
        eprintln!("SKIP nowrite: ngspice not installed");
        return;
    }
    let before = tree_listing(&cases);
    let (out, json, _md, _art) = run_suite(&cases, &[]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(tree_listing(&cases), before, "the run changed the corpus");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    // the include chain still reaches ngspice through the links
    let c = case(&v, "nowrite_chain");
    assert_eq!(c["ngspice_status"], "ok", "{c:#?}");
    assert_eq!(c["ref_status"], "tight", "{c:#?}");
}

#[test]
fn tstart_window_and_row_count_are_compared_correctly() {
    let tstart = "Transient with a start time
* bench.id: tr_tstart
* bench.group: capability
* bench.probes: v(out)
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u 2u
.end
";
    let partial = "tstop not a multiple of tstep
* bench.id: tr_partial
* bench.group: accuracy
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, npoints
V1 in 0 SIN(0 1 100k)
R1 in out 100
C1 out 0 10p
.tran 1u 10.5u
.end
";
    let big = "tstep larger than tstop
* bench.id: tr_big_step
* bench.group: robustness
* bench.probes: v(2)
* bench.expect: either
V1 1 0 SIN(0 1 100meg)
R1 1 2 1k
C1 2 0 1p
.tran 1u 1n
.end
";
    let Some((v, _)) = run_one(
        "tran_grid",
        &[
            ("tr_tstart.sp", tstart.as_bytes()),
            ("tr_partial.sp", partial.as_bytes()),
            ("tr_big_step.sp", big.as_bytes()),
        ],
    ) else {
        return;
    };
    // rustspice emits from t = 0, ngspice only from tstart: the window is [first ngspice row, tstop]
    let c = case(&v, "tr_tstart");
    assert_eq!(c["status"], "pass", "{c:#?}");
    assert!(c["notes"].to_string().contains("tstart honoured"), "{c:#?}");
    // the final partial step is part of the grid and the row count is checked
    let c = case(&v, "tr_partial");
    assert_eq!(c["status"], "pass", "{c:#?}");
    assert!(
        c["notes"]
            .to_string()
            .contains("npoints: rustspice wrote 12 rows"),
        "{c:#?}"
    );
    // tstep > tstop: the grid comes from the rustspice time rows, not from a degenerate nominal grid
    let c = case(&v, "tr_big_step");
    assert!(
        c["notes"]
            .to_string()
            .contains("compared at the rustspice time rows"),
        "{c:#?}"
    );
    assert!(
        !c["notes"].to_string().contains("no common time range"),
        "{c:#?}"
    );
}

const ISLAND: &str = "Floating island
* bench.id: island
* bench.group: robustness
* bench.probes: v(1)
* bench.expect: either
* bench.tags: TAGS
V1 1 0 5
R1 1 0 1k
V2 3 4 2
R3 3 4 1k
.op
.end
";

#[test]
fn ngspice_error_ok_deck_with_singular_warnings_is_a_run() {
    // ngspice-47 logs `Warning: singular matrix` / gmin-stepping notes, exits 0 and writes a rawfile; rustspice
    // refuses (caught panic noise on stderr, exit 1). The log-pattern tolerance itself is unit-tested with a fake
    // ngspice (ngspice.rs); here the real tool shows the case is a normal run and a refusal is not a panic.
    let with = ISLAND.replace("TAGS", "caught_panic_noise, ngspice_error_ok");
    let Some((v, _)) = run_one("error_ok", &[("island.sp", with.as_bytes())]) else {
        return;
    };
    let c = case(&v, "island");
    assert_eq!(c["ngspice_status"], "ok", "{c:#?}");
    assert_eq!(c["rustspice"]["class"], "loud_error", "{c:#?}");
    assert_eq!(c["status"], "info", "{c:#?}");
    assert_eq!(c["outcome"], "loud_error");
    if c["rustspice"]["caught_panic_noise"] == true {
        assert!(
            c["notes"].to_string().contains("caught panic message"),
            "{c:#?}"
        );
    }
}

#[test]
fn ngspice_unbounded_decks_never_start_ngspice() {
    let deck = "Pretend resource bomb
* bench.id: bomb
* bench.group: robustness
* bench.expect: either
* bench.known_gap: resource-budget
* bench.gap.note: a pretend resource bomb that RustSpice now finishes
* bench.tags: hostile, ngspice_unbounded
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
";
    let Some((v, art)) = run_one("unbounded", &[("bomb.sp", deck.as_bytes())]) else {
        return;
    };
    let c = case(&v, "bomb");
    assert_eq!(c["ngspice_status"], "skipped_unbounded", "{c:#?}");
    assert_eq!(c["ref_status"], "skipped_unbounded");
    assert_eq!(c["ngspice"]["status"], "skipped_unbounded");
    assert_eq!(c["rustspice"]["class"], "ok");
    // judged against `expect` only: rustspice finished, so the budget gap is closed
    assert_eq!(c["status"], "gap_closed", "{c:#?}");
    assert!(!art.join("bomb").join("ng_sparse.sp").exists());
}

#[test]
fn sidecar_metadata_and_ngspice_crash_retry() {
    // a zero-byte deck cannot carry metadata; ngspice-47 segfaults on it (retried once), rustspice refuses
    let side = "bench.id: empty_deck\nbench.group: robustness\nbench.expect: loud_error\nbench.tags: file, ngspice_crash\n";
    let Some((v, _)) = run_one(
        "sidecar",
        &[
            ("empty_deck.sp", b""),
            ("empty_deck.sp.meta", side.as_bytes()),
        ],
    ) else {
        return;
    };
    let c = case(&v, "empty_deck");
    assert_eq!(c["status"], "pass", "{c:#?}");
    assert_eq!(c["outcome"], "loud_error");
    let ng = c["ngspice_status"].as_str().unwrap();
    assert!(
        ng.contains("signal 11") && ng.contains("retried once"),
        "{ng}"
    );
    assert!(c["notes"].to_string().contains("sidecar"));
}

#[test]
fn non_utf8_decks_reach_both_tools_byte_exact() {
    let deck: &[u8] = b"latin-1 title \xb5\xe9
* bench.id: latin1
* bench.group: robustness
* bench.probes: v(1), v(2)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: refuses non-UTF-8 text
* comment \xb5 \xe9
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
";
    let Some((v, art)) = run_one("latin1", &[("latin1.sp", deck)]) else {
        return;
    };
    let c = case(&v, "latin1");
    assert_eq!(c["ngspice_status"], "ok", "{c:#?}");
    assert_eq!(c["rustspice"]["class"], "loud_error");
    assert_eq!(c["status"], "known_gap_confirmed", "{c:#?}");
    assert_eq!(
        std::fs::read(art.join("latin1").join("deck.sp")).unwrap(),
        deck
    );
}

// ---------------------------------------------------------------------------------------------------------
// E2: peak RSS, --teeth, performance gaps, the resampling floor, timing mode (SUITE_SPEC 14.3)
// ---------------------------------------------------------------------------------------------------------

/// `run_suite` writes its outputs and artifacts next to the decks; a second run in the same directory must not
/// read the first run's kept decks as cases.
fn clean_outputs(cases: &Path) {
    let _ = std::fs::remove_dir_all(cases.join("artifacts"));
    let _ = std::fs::remove_file(cases.join("out.json"));
    let _ = std::fs::remove_file(cases.join("out.md"));
}

fn load(json: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap()
}

fn kib(v: &Value) -> Option<u64> {
    v.as_u64()
}

#[test]
fn a_tiny_childs_peak_rss_is_its_own_not_the_harness_footprint() {
    if !ngspice_available() {
        eprintln!("SKIP a_tiny_childs_peak_rss: ngspice not installed");
        return;
    }
    // `a_big_tran` makes the harness itself hold >100 MB of waveforms (1e6 rows read from two rawfiles); `z_tiny_op`
    // runs afterwards (cases run in id order). `wait4`'s ru_maxrss of its children would report the harness's
    // high-water mark for every child; VmHWM polling reports each child's own.
    let big = "Big transient to fatten the harness
* bench.id: a_big_tran
* bench.group: accuracy
* bench.probes: v(out)
* bench.ref: none
* bench.tol.norm: 5e-2
* bench.timing: no
V1 in 0 SIN(0 1 1k)
R1 in out 1k
C1 out 0 1n
.tran 100n 100m
.end
";
    let tiny = "Tiny op
* bench.id: z_tiny_op
* bench.group: accuracy
* bench.probes: v(out)
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
.op
.end
";
    let dir = scratch("rss_tiny");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "a_big_tran.sp", big);
    put(&cases, "z_tiny_op.sp", tiny);
    let (out, json, _md, _art) = run_suite(&cases, &[]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    let tiny_c = case(&v, "z_tiny_op");
    let big_c = case(&v, "a_big_tran");
    assert_eq!(tiny_c["status"], "pass", "{tiny_c:#?}");
    let limit = 64 * 1024; // KiB: a harness that is >100 MB would show up here
    let rs_tiny = kib(&tiny_c["rustspice"]["rss_kib"]).expect("rustspice peak RSS measured");
    let ng_tiny = kib(&tiny_c["ngspice"]["rss_kib"]).expect("ngspice peak RSS measured");
    assert!(rs_tiny > 0 && rs_tiny < limit, "rustspice {rs_tiny} KiB");
    assert!(ng_tiny > 0 && ng_tiny < limit, "ngspice {ng_tiny} KiB");
    assert_eq!(tiny_c["rustspice"]["rss_status"], "measured_vmhwm");
    assert_eq!(tiny_c["ngspice"]["rss_status"], "measured_vmhwm");
    // the children differ from each other (the old numbers were identical for all three)
    assert_ne!(rs_tiny, ng_tiny);
    for ch in tiny_c["rss"]["children"].as_array().unwrap() {
        let p = ch["peak_kib"].as_u64().expect("child measured");
        assert!(p < limit, "{ch}");
    }
    // and the big case really is bigger for RustSpice (it holds 1e6 rows), so the number tracks the child
    if let Some(rs_big) = kib(&big_c["rustspice"]["rss_kib"]) {
        assert!(rs_big > 2 * rs_tiny, "big {rs_big} vs tiny {rs_tiny}");
    }
    // the case-level report names the largest child
    assert!(tiny_c["rss"]["max_child"].is_string());
    assert_eq!(
        tiny_c["rss"]["max_kib"].as_u64().unwrap(),
        tiny_c["rss"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["peak_kib"].as_u64())
            .max()
            .unwrap()
    );
}

#[test]
fn the_rss_estimate_is_compared_with_the_rustspice_child_alone() {
    if !ngspice_available() {
        eprintln!("SKIP the_rss_estimate: ngspice not installed");
        return;
    }
    // ngspice's start-up size (~14 MiB) is more than twice rustspice's (~6 MiB). The estimate is RustSpice's peak:
    // 3 MiB is exceeded by RustSpice itself, 8 MiB is NOT (6.3 MiB x 1 is inside the 25 % slack even though the
    // ngspice children are above it - the old maximum over all children called every small deck "exceeding"), 200 MiB
    // is not, no estimate is reported as such
    let mk = |id: &str, est: Option<u64>| {
        format!(
            "Divider {id}\n* bench.id: {id}\n* bench.group: accuracy\n* bench.probes: v(out)\n{}V1 in 0 DC 5\nR1 in out 1k\nR2 out 0 3k\n.op\n.end\n",
            est.map_or(String::new(), |e| format!("* bench.est_rss_mib: {e}\n"))
        )
    };
    let dir = scratch("rss_est");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "est_small.sp", &mk("est_small", Some(3)));
    put(&cases, "est_mid.sp", &mk("est_mid", Some(8)));
    put(&cases, "est_big.sp", &mk("est_big", Some(200)));
    put(&cases, "est_none.sp", &mk("est_none", None));
    let (out, json, md, _art) = run_suite(&cases, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    assert_eq!(
        case(&v, "est_small")["rss"]["est_status"],
        "exceeds_estimate"
    );
    let mid = &case(&v, "est_mid")["rss"];
    let ng_kib = mid["ngspice_max_kib"]
        .as_u64()
        .expect("ngspice peak reported separately");
    let rs_kib = mid["rustspice_kib"].as_u64().expect("rustspice peak");
    if cfg!(debug_assertions) {
        // an unoptimised RustSpice starts at about 17 MiB, above ngspice's 14 MiB, so no estimate separates the two
        eprintln!(
            "the 'estimate vs rustspice alone' premise needs an optimised build (rustspice {rs_kib} KiB, ngspice {ng_kib} KiB): run `cargo test --release`"
        );
    } else {
        assert!(
            ng_kib > 8 * 1024 * 5 / 4,
            "the test needs ngspice above the 8 MiB estimate's slack ({ng_kib} KiB)"
        );
        assert!(rs_kib <= 8 * 1024 * 5 / 4, "rustspice {rs_kib} KiB");
        assert_eq!(mid["est_status"], "within_estimate", "{mid}");
    }
    assert_eq!(case(&v, "est_big")["rss"]["est_status"], "within_estimate");
    assert_eq!(case(&v, "est_none")["rss"]["est_status"], "no_estimate");
    assert_eq!(case(&v, "est_small")["rss"]["est_mib"], 3);
    // the status is never changed by a memory remark
    assert_eq!(case(&v, "est_small")["status"], "pass");
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(
        text.contains("Peak RSS versus `bench.est_rss_mib`"),
        "{text}"
    );
    assert!(text.contains("`est_small`"));
}

const DIVIDER: &str = "Divider
* bench.id: ID
* bench.group: accuracy
* bench.probes: v(out)
* bench.tol.rel: TOLREL
* bench.tol.abs: 1e-9
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
R3 out x 10k
R4 x 0 10k
.op
.end
";

#[test]
fn teeth_separates_sharp_blunt_skipped_and_not_applicable_cases() {
    if !ngspice_available() {
        eprintln!("SKIP teeth: ngspice not installed");
        return;
    }
    let dir = scratch("teeth");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    // sharp: 1e-6 tolerance sees a 5 % change of V1 (and of every resistor)
    put(
        &cases,
        "t_sharp.sp",
        &DIVIDER.replace("ID", "t_sharp").replace("TOLREL", "1e-6"),
    );
    // blunt (tolerance): a 100 % relative tolerance cannot see 5 %
    put(
        &cases,
        "t_loose.sp",
        &DIVIDER.replace("ID", "t_loose").replace("TOLREL", "1.0"),
    );
    // blunt (independence): v(a) is held by V1 alone; the first, middle and last element are resistors of another net
    put(
        &cases,
        "t_indep.sp",
        "Probe not connected to the mutated elements
* bench.id: t_indep
* bench.group: accuracy
* bench.probes: v(a)
* bench.tol.rel: 1e-6
R1 b 0 1k
V1 a 0 1
R2 b c 1k
R3 c d 1k
R4 d 0 1k
.op
.end
",
    );
    // skipped: every value is a parameter expression
    put(
        &cases,
        "t_param.sp",
        "All values behind parameters
* bench.id: t_param
* bench.group: accuracy
* bench.probes: v(out)
.param rv=1k
.param vs=5
V1 in 0 DC {vs}
R1 in out {rv}
R2 out 0 {rv}
.op
.end
",
    );
    // n/a: a refusal is expected, nothing numeric is compared
    put(
        &cases,
        "t_refuse.sp",
        "Refusal
* bench.id: t_refuse
* bench.group: robustness
* bench.expect: loud_error
V1 a 0 DC 1
R2 a b
.op
.end
",
    );
    // n/a: known gap, not a pass
    put(
        &cases,
        "t_gap.sp",
        &CLAMP_STACK.replace("clamp_rail", "t_gap"),
    );
    let (out, json, md, art) = run_suite(&cases, &["--teeth"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    let teeth = |id: &str| case(&v, id)["teeth"].clone();

    // `status` is never changed by the check
    for id in ["t_sharp", "t_loose", "t_indep"] {
        assert_eq!(case(&v, id)["status"], "pass", "{id}");
    }
    let sharp = teeth("t_sharp");
    assert_eq!(sharp["status"], "ok", "{sharp:#}");
    assert!(sharp["mutations"].as_array().unwrap().len() >= 2);
    let names: Vec<&str> = sharp["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["element"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["V1", "R2", "R4"],
        "first, middle and last element: {sharp:#}"
    );
    for m in sharp["mutations"].as_array().unwrap() {
        assert_eq!(m["rustspice"], "ok");
        assert_eq!(m["outcome"], "detected", "{m}");
        assert!(m["worst_tol_ratio"].as_f64().unwrap() > 1.0);
        assert!(m["response"].as_f64().unwrap() > 1e-5, "{m}");
    }
    assert_eq!(sharp["mutations"][0]["from"], "5");
    assert_eq!(sharp["mutations"][0]["param"], "dc");

    let loose = teeth("t_loose");
    assert_eq!(loose["status"], "blunt", "{loose:#}");
    assert_eq!(loose["reason_class"], "tolerance_too_loose");
    for m in loose["mutations"].as_array().unwrap() {
        assert_eq!(m["outcome"], "survived");
        assert!(m["worst_tol_ratio"].as_f64().unwrap() < 1.0);
        assert!(
            m["response"].as_f64().unwrap() > 1e-5,
            "the probes DO move: {m}"
        );
    }

    let indep = teeth("t_indep");
    assert_eq!(indep["status"], "blunt", "{indep:#}");
    assert_eq!(indep["reason_class"], "probe_independent");
    for m in indep["mutations"].as_array().unwrap() {
        assert!(m["response"].as_f64().unwrap() < 1e-9, "{m}");
    }

    let skipped = teeth("t_param");
    assert_eq!(skipped["status"], "skipped", "{skipped:#}");
    assert_eq!(skipped["reason_class"], "no_mutable_element");
    assert!(skipped["reason"]
        .as_str()
        .unwrap()
        .contains("expression_or_param_value"));

    let refused = teeth("t_refuse");
    assert_eq!(refused["status"], "n/a");
    assert_eq!(refused["reason_class"], "no_numeric_comparison");
    let gap = teeth("t_gap");
    assert_eq!(gap["status"], "n/a");
    assert_eq!(gap["reason_class"], "not_a_pass");

    // summary + Markdown
    let s = &v["teeth"];
    assert_eq!(
        (s["ok"].as_u64(), s["blunt"].as_u64(), s["skipped"].as_u64()),
        (Some(1), Some(2), Some(1))
    );
    assert_eq!(s["n_a"], 2);
    assert_eq!(s["blunt_by_reason"]["probe_independent"], 1);
    assert_eq!(s["blunt_by_reason"]["tolerance_too_loose"], 1);
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains("### Mutation sensitivity"), "{text}");
    assert!(text.contains("`t_loose`") && text.contains("`t_indep`"));
    // the mutated decks of a blunt case are kept and differ from the original in exactly one line
    let orig = std::fs::read_to_string(cases.join("t_loose.sp")).unwrap();
    let mutated = std::fs::read_to_string(art.join("t_loose").join("teeth_0.sp")).unwrap();
    let diff: Vec<_> = orig
        .lines()
        .zip(mutated.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(orig.lines().count(), mutated.lines().count());
    assert!(diff[0].1.starts_with("V1 in 0 DC 5.25"), "{diff:?}");
    assert!(art.join("t_loose").join("teeth.txt").exists());
    assert!(!art.join("t_sharp").exists(), "ok cases keep nothing");

    // without the flag nothing is reported (null, not a fake verdict)
    clean_outputs(&cases);
    let (out2, json2, _md2, _art2) = run_suite(&cases, &[]);
    assert_eq!(out2.status.code(), Some(0));
    let v2 = load(&json2);
    assert!(case(&v2, "t_sharp")["teeth"].is_null());
    assert!(v2["teeth"].is_null());
}

#[test]
fn a_performance_gap_needs_timing_and_is_judged_end_to_end() {
    if !ngspice_available() {
        eprintln!("SKIP a_performance_gap: ngspice not installed");
        return;
    }
    let deck = "Tiny divider tagged as a performance gap
* bench.id: perf_gap_tiny
* bench.group: performance
* bench.probes: v(out)
* bench.known_gap: faer-nonlinear-slow
* bench.gap.note: pretend the faer tier is slow
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
.op
.end
";
    let dir = scratch("perfgap");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "perf_gap_tiny.sp", deck);
    // no timing: nothing can be said about a speed claim
    let (out, json, _md, _art) = run_suite(&cases, &["--no-timing"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let c = case(&load(&json), "perf_gap_tiny").clone();
    assert_eq!(c["status"], "unverified", "{c:#}");
    assert_eq!(c["timing_status"], "disabled");
    assert!(c["notes"].to_string().contains("not judged"), "{c:#}");
    // with timing the verdict comes from the END-TO-END median: rustspice starts ~3x faster than ngspice
    clean_outputs(&cases);
    let (out, json, _md, _art) = run_suite(&cases, &["--reps", "5"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let c = case(&load(&json), "perf_gap_tiny").clone();
    assert_eq!(c["timing_status"], "measured", "{c:#}");
    let e2e = c["timing"]["headline"]["e2e_median"].as_f64().unwrap();
    let notes = c["notes"].to_string();
    assert!(notes.contains("end-to-end median speedup"), "{notes}");
    assert!(
        notes.contains("solve-only"),
        "the solve-only ratio is informational: {notes}"
    );
    if e2e >= 1.1 && c["timing"]["noisy"] == false {
        assert_eq!(c["status"], "gap_closed", "{c:#}");
    } else {
        // a noisy or parity measurement never closes a gap
        assert_eq!(c["status"], "known_gap_confirmed", "{c:#}");
    }
}

#[test]
fn the_resampling_floor_note_only_exists_where_it_decides_something() {
    if !ngspice_available() {
        eprintln!("SKIP resampling floor: ngspice not installed");
        return;
    }
    let deck = |id: &str, extra: &str, tol: &str| {
        format!(
            "RC low-pass, 1 kHz sine\n* bench.id: {id}\n* bench.group: accuracy\n* bench.probes: v(out)\n* bench.tol.norm: {tol}\n{extra}V1 in 0 SIN(0 1 1k)\nR1 in out 1k\nC1 out 0 100n\n.tran 5u 2m\n.end\n"
        )
    };
    let dir = scratch("floor");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    // dense reference (tmax = tstep/20): the floor is far below the tolerance, a plain pass without any remark
    put(&cases, "fl_dense.sp", &deck("fl_dense", "", "5e-3"));
    // a reference with 40 rows per period (tmax 25 us) and a 1e-4 tolerance: the reference's own resampling noise
    // is above the tolerance and the error is inside that noise: unverified, with the reason
    put(
        &cases,
        "fl_coarse.sp",
        &deck("fl_coarse", "* bench.ref.tmax: 25u\n", "1e-4"),
    );
    let (out, json, _md, _art) = run_suite(&cases, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    let dense = case(&v, "fl_dense");
    assert_eq!(dense["status"], "pass", "{dense:#}");
    assert!(
        !dense["notes"].to_string().contains("resampling floor"),
        "never on a pass: {dense:#}"
    );
    let coarse = case(&v, "fl_coarse");
    let notes = coarse["notes"].to_string();
    let p = &coarse["probes"][0];
    let (fl, e) = (
        p["interp_floor"].as_f64().unwrap(),
        p["norm_err"].as_f64().unwrap(),
    );
    if fl > 1e-4 && (e - 1e-4).abs() < fl {
        assert_eq!(coarse["status"], "unverified", "{coarse:#}");
        assert!(
            notes.contains("resampling floor") && notes.contains("cannot decide"),
            "{notes}"
        );
    } else if e >= 1e-4 + fl {
        // clearly failing: a fail never carries the remark
        assert_eq!(coarse["status"], "fail_accuracy", "{coarse:#}");
        assert!(!notes.contains("resampling floor"), "{notes}");
    } else {
        assert_eq!(coarse["status"], "pass", "{coarse:#}");
        assert!(!notes.contains("resampling floor"), "{notes}");
    }
}

#[test]
fn timing_mode_on_a_custom_corpus_with_an_include_leaves_the_corpus_untouched() {
    if !ngspice_available() {
        eprintln!("SKIP timing mode: ngspice not installed");
        return;
    }
    let dir = scratch("timing_inc");
    let cases = dir.join("cases");
    std::fs::create_dir_all(cases.join("lib")).unwrap();
    // a nested include chain through a sub-directory; the model is never used, so rustspice (which ignores
    // `.include`) and ngspice (which needs the files) must agree
    put(
        &cases,
        "perf_inc.sp",
        "Timed divider with an include chain
* bench.id: perf_inc
* bench.group: performance
* bench.probes: v(out)
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
.include lib/outer.lib
.op
.end
",
    );
    put(&cases.join("lib"), "outer.lib", ".include inner.lib\n");
    put(
        &cases.join("lib"),
        "inner.lib",
        ".model DUNUSED D(IS=1e-12)\n",
    );
    put(&cases, "outer.lib", ".model DX D(IS=1e-9)\n");
    let before = tree_listing(&cases);
    let (out, json, _md, _art) = run_suite(&cases, &["--reps", "5"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        tree_listing(&cases),
        before,
        "a timed run must not write into the corpus"
    );
    assert!(!PathBuf::from("b3v33check.log").exists());
    let v = load(&json);
    let c = case(&v, "perf_inc");
    assert_eq!(c["status"], "pass", "{c:#}");
    assert_eq!(c["ngspice_status"], "ok");
    assert_eq!(c["timing_status"], "measured", "{c:#}");
    let t = &c["timing"];
    assert_eq!(t["reps_done"], 5);
    assert!(t["loadavg_before"].as_f64().is_some() && t["loadavg_after"].as_f64().is_some());
    assert!(t["noisy"].is_boolean());
    for k in t["contenders"].as_array().unwrap() {
        let e = &k["e2e_us"];
        assert_eq!(e["n"], 5, "{k}");
        for f in ["min", "median", "mean", "stddev"] {
            assert!(e[f].as_f64().unwrap() >= 0.0, "{f} in {k}");
        }
        assert!(e["min"].as_f64().unwrap() <= e["median"].as_f64().unwrap());
        assert!(e["noisy"].is_boolean());
        assert_eq!(e["samples"].as_array().unwrap().len(), 5);
        assert_eq!(k["rss_status"], "measured_vmhwm", "{k}");
        assert!(k["rss_kib"]["min"].as_f64().unwrap() > 0.0);
    }
    // ratios are sane: a trivial deck is dominated by start-up (ngspice ~4 ms, rustspice ~1 ms)
    let x = t["headline"]["e2e_median"].as_f64().unwrap();
    assert!((0.2..200.0).contains(&x), "implausible speedup {x}");
    assert!(v["startup_floor"]["status"] == "ok");
}

/// First CPU this process may run on, at the end of the list (the busiest cores are usually the low ones).
fn some_allowed_cpu() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let list = status
        .lines()
        .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))?
        .trim();
    list.split(',')
        .filter_map(|r| {
            let mut it = r.split('-');
            let a: usize = it.next()?.trim().parse().ok()?;
            let b: usize = it.next().map_or(Some(a), |x| x.trim().parse().ok())?;
            Some(b.max(a))
        })
        .max()
}

/// Timing mode pins the children to one core, which is exactly when a `/proc/<pid>/status` read right after `spawn`
/// can still show the harness's own address space. A fat harness (the big transient before it) must not leak its
/// footprint into the timed children's peak RSS.
#[test]
fn pinned_timing_reports_each_childs_own_peak_rss_even_from_a_fat_harness() {
    if !ngspice_available() {
        eprintln!("SKIP pinned timing RSS: ngspice not installed");
        return;
    }
    let Some(cpu) = some_allowed_cpu() else {
        eprintln!("SKIP pinned timing RSS: no readable Cpus_allowed_list");
        return;
    };
    let big = "Big transient to fatten the harness
* bench.id: a_big_tran
* bench.group: accuracy
* bench.probes: v(out)
* bench.ref: none
* bench.tol.norm: 5e-2
* bench.timing: no
V1 in 0 SIN(0 1 1k)
R1 in out 1k
C1 out 0 1n
.tran 100n 100m
.end
";
    let tiny = "Timed tiny divider
* bench.id: z_perf_tiny
* bench.group: performance
* bench.probes: v(out)
V1 in 0 DC 5
R1 in out 1k
R2 out 0 3k
.op
.end
";
    let dir = scratch("rss_pinned");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "a_big_tran.sp", big);
    put(&cases, "z_perf_tiny.sp", tiny);
    clean_outputs(&cases);
    let cpu_s = cpu.to_string();
    let (out, json, _md, _art) = run_suite(&cases, &["--reps", "5", "--pin-cpu", &cpu_s]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    let c = case(&v, "z_perf_tiny");
    assert_eq!(c["timing_status"], "measured", "{c:#}");
    let limit = 64.0 * 1024.0; // KiB
    let mut seen = std::collections::BTreeSet::new();
    for k in c["timing"]["contenders"].as_array().unwrap() {
        assert_eq!(k["rss_status"], "measured_vmhwm", "{k}");
        let r = &k["rss_kib"];
        let lo = r["min"].as_f64().unwrap();
        let hi = r["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_f64())
            .fold(f64::MIN, f64::max);
        assert!(
            lo > 0.0 && hi < limit,
            "{}: peak RSS {lo}..{hi} KiB must be the child's own, not the fat harness's",
            k["name"]
        );
        seen.insert(hi as u64);
    }
    // the three children are different programs: their numbers are not one shared footprint
    assert!(
        seen.len() >= 2,
        "identical RSS for all contenders: {seen:?}"
    );
}

/// A refusal that echoes a 100 KB token must not put 100 KB (times the copies the report keeps) into the report.
#[test]
fn a_huge_refusal_message_is_clipped_in_the_report() {
    if !ngspice_available() {
        eprintln!("SKIP huge refusal message: ngspice not installed");
        return;
    }
    let dir = scratch("huge_msg");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    let deck = format!(
        "Deeply nested expression as a resistance
* bench.id: huge_msg
* bench.group: robustness
* bench.expect: either
* bench.timing: no
V1 in 0 DC 5
R1 in out {{{}1{}}}
R2 out 0 1k
.op
.end
",
        "(".repeat(50_000),
        ")".repeat(50_000)
    );
    put(&cases, "huge_msg.sp", &deck);
    clean_outputs(&cases);
    let (out, json, _md, _art) = run_suite(&cases, &[]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&json).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    let c = case(&v, "huge_msg");
    assert_eq!(c["rustspice"]["class"], "loud_error", "{c:#}");
    let msg = c["rustspice"]["message"].as_str().unwrap();
    assert!(msg.starts_with("exit 1: Error: ParseError"), "{msg}");
    assert!(msg.len() < 600, "message not clipped: {} bytes", msg.len());
    assert!(msg.contains("more bytes]"), "{msg}");
    assert!(text.len() < 60_000, "report grew to {} bytes", text.len());
}

// ---------------------------------------------------------------------------------------------------------
// Engine hardening: failure modes of the harness itself (scratch, environment, signals, arguments)
// ---------------------------------------------------------------------------------------------------------

/// A `/bin/sh` stand-in for ngspice: answers `--version` like the real one, runs `body` for everything else.
fn fake_ngspice(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(
        &p,
        format!("#!/bin/sh\ncase \"$1\" in --version) echo '** ngspice-99 : fake'; exit 0;; esac\n{body}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// `Output` of a command that is killed when it exceeds `secs` (a regression must fail the test, not hang it).
fn output_within(mut cmd: Command, secs: u64) -> Output {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let start = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if start.elapsed() > Duration::from_secs(secs) {
            let _ = child.kill();
            let out = child.wait_with_output().unwrap();
            panic!(
                "timed out after {secs} s\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_file(path: &Path, secs: u64) -> String {
    let start = Instant::now();
    loop {
        if let Ok(t) = std::fs::read_to_string(path) {
            if t.ends_with('\n') {
                return t;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(secs),
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A process that has ended (or is only a zombie waiting to be reaped).
fn process_gone(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        Ok(s) => s
            .rsplit(')')
            .next()
            .is_some_and(|rest| rest.trim_start().starts_with('Z')),
    }
}

fn wait_until(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    f()
}

fn scratch_dirs_of(pid: u32) -> Vec<PathBuf> {
    [PathBuf::from("/dev/shm"), std::env::temp_dir()]
        .into_iter()
        .map(|b| b.join(format!("rustspice-bench-{pid}-suite")))
        .filter(|p| p.exists())
        .collect()
}

fn suite_cmd(cases: &Path) -> Command {
    let mut c = Command::new(BIN);
    c.arg("bench-suite").arg("--cases").arg(cases).args([
        "--no-fuzz",
        "--family",
        "static",
        "--scale",
        "small",
        "--no-timing",
    ]);
    c
}

/// ER-driver-01: with a full or failing scratch file system both tools were handed an empty deck, every `loud_error`
/// deck "passed" and the run was a PASS with exit 0. Same code path as ENOSPC on /dev/shm: a file size limit of 0.
#[test]
fn a_failing_scratch_file_system_makes_the_run_unverified_never_a_pass() {
    if !ngspice_available() {
        eprintln!(
            "SKIP scratch failure: ngspice not installed (the run would be unverified anyway)"
        );
        return;
    }
    let cases = scratch("fsize");
    put(
        &cases,
        "zz_loud_ok.sp",
        REFUSE.replace("refuse_malformed", "zz_loud_ok").as_str(),
    );
    put(&cases, "zz_ok.sp", &RC_OP.replace("rc_op", "zz_ok"));
    // a valid deck wrongly tagged as a refusal: it must NOT pass because both tools got an empty file
    put(
        &cases,
        "zz_valid_loud.sp",
        &RC_OP.replace("rc_op", "zz_valid_loud").replace(
            "* bench.group: accuracy",
            "* bench.group: robustness\n* bench.expect: loud_error",
        ),
    );
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("trap '' XFSZ; ulimit -f 0; exec \"$0\" \"$@\"")
        .arg(BIN)
        .arg("bench-suite")
        .arg("--cases")
        .arg(&cases)
        .args([
            "--no-fuzz",
            "--family",
            "static",
            "--scale",
            "small",
            "--no-timing",
        ]);
    let out = output_within(cmd, 120);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("scratch write failed"), "{err}");
    assert!(
        err.contains("scratch file system is full or read-only"),
        "{err}"
    );
    assert!(!err.contains(" pass ("), "no case may pass:\n{err}");
    assert!(!err.contains("verdict: PASS"), "{err}");
}

/// ER-driver-02: `ng_installed` meant "`--version` could be spawned"; a fake or broken ngspice made every case
/// `ngspice_error` (never failing) and the loud-error decks passed on RustSpice alone.
#[test]
fn an_ngspice_that_cannot_simulate_makes_the_run_unverified() {
    let dir = scratch("fakeng");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "rc_op.sp", RC_OP);
    put(&cases, "refuse_malformed.sp", REFUSE);
    // (a) programs that are not ngspice: `--version` fails (false) or does not say ngspice (true)
    for prog in ["/bin/false", "/bin/true", "/nonexistent/ngspice"] {
        let (out, json, md, _art) = run_suite(&cases, &["--ngspice", prog]);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{prog}: {err}");
        let v = load(&json);
        assert_eq!(v["ngspice"]["installed"], false, "{prog}");
        assert!(
            v["ngspice"]["unusable_reason"].is_string() || prog.contains("nonexistent"),
            "{prog}"
        );
        assert_eq!(case(&v, "rc_op")["status"], "unverified", "{prog}");
        assert_eq!(
            case(&v, "refuse_malformed")["status"],
            "unverified",
            "{prog}: a refusal deck must not pass on RustSpice alone"
        );
        let text = std::fs::read_to_string(&md).unwrap();
        assert!(text.contains("UNVERIFIED"), "{prog}");
    }
    // (b) a program that passes the version probe but never produces a result
    let ng = fake_ngspice(
        &dir,
        "ng_dead",
        "echo 'Error: no simulation today' >&2\nexit 1",
    );
    let (out, json, _md, _art) = run_suite(&cases, &["--ngspice", ng.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    let v = load(&json);
    assert_eq!(v["summary"]["verdict"], "UNVERIFIED", "{err}");
    assert_ne!(case(&v, "rc_op")["status"], "pass");
    // an explicitly accepted all-unverified run exits 0
    let (out, _json, _md, _art) = run_suite(
        &cases,
        &["--ngspice", ng.to_str().unwrap(), "--allow-unverified"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// ER-driver-03 and 12: a mistyped `--cases` was a green run of the generated decks; two self-referencing symlinks
/// made the walk exponential.
#[test]
fn a_mistyped_cases_directory_or_a_symlink_loop_is_handled() {
    let dir = scratch("badcases");
    let mut cmd = Command::new(BIN);
    cmd.arg("bench-suite")
        .arg("--cases")
        .arg(dir.join("does_not_exist"))
        .args([
            "--no-fuzz",
            "--scale",
            "small",
            "--no-timing",
            "--filter",
            "fam_ladder_dc_200",
        ]);
    let out = output_within(cmd, 60);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("is not a directory"), "{err}");
    // the generated decks alone only on request
    let mut cmd = Command::new(BIN);
    cmd.arg("bench-suite")
        .arg("--cases")
        .arg(dir.join("does_not_exist"))
        .args([
            "--no-fuzz",
            "--scale",
            "small",
            "--families-only",
            "--list",
            "--filter",
            "fam_ladder_dc_200",
        ]);
    let out = output_within(cmd, 60);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("fam_ladder_dc_200"));
    // symbolic links to directories are not followed (a loop would never end)
    let loops = dir.join("loops");
    std::fs::create_dir_all(&loops).unwrap();
    put(&loops, "rc_op.sp", RC_OP);
    std::os::unix::fs::symlink(".", loops.join("l1")).unwrap();
    std::os::unix::fs::symlink(".", loops.join("l2")).unwrap();
    let mut cmd = suite_cmd(&loops);
    cmd.arg("--list");
    let out = output_within(cmd, 60);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).lines().count(), 1);
    assert!(err.contains("not followed"), "{err}");
}

/// ER-driver-05: an `unverified` case never changed the verdict line, the Markdown headline or the exit code.
#[test]
fn unverified_cases_are_visible_and_strict_mode_fails_the_run() {
    if !ngspice_available() {
        eprintln!("SKIP unverified visibility: ngspice not installed");
        return;
    }
    let dir = scratch("unv");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "rc_op.sp", RC_OP);
    put(&cases, "bad_meta.sp", BAD_META);
    let (out, json, md, _art) = run_suite(&cases, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    let v = load(&json);
    // the JSON verdict vocabulary is unchanged; the new fields carry the detail
    assert_eq!(v["summary"]["verdict"], "PASS");
    assert_eq!(v["summary"]["unverified_count"], 1);
    let line = v["summary"]["verdict_line"].as_str().unwrap();
    assert!(line.starts_with("PASS (but 1 unverified"), "{line}");
    let u = &v["summary"]["unverified"][0];
    assert_eq!(u["id"], "bad_meta");
    assert!(
        u["reason"]
            .as_str()
            .unwrap()
            .contains("match|loud_error|either"),
        "{u}"
    );
    assert!(err.contains("verdict: PASS (but 1 unverified"), "{err}");
    assert!(err.contains("unverified: bad_meta"), "{err}");
    let text = std::fs::read_to_string(&md).unwrap();
    let head = text.split("## 2.").next().unwrap();
    assert!(head.contains("PASS (but 1 unverified"), "{head}");
    assert!(
        head.contains("bad_meta") && head.contains("`maybe` is not"),
        "{head}"
    );
    // strict mode: exit 2, same report
    let (out, json2, _md2, _art2) = run_suite(&cases, &["--strict-unverified"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("--strict-unverified"), "{err}");
    assert_eq!(load(&json2)["summary"]["verdict"], "PASS");
}

/// ER-driver-18, ER-driver-13 and ER-process-06: numeric arguments are validated before anything runs;
/// `--pin-cpu 5000` aborted the harness (SIGABRT, exit 134) and `--pin-cpu 999` was reported as "pinned".
#[test]
fn numeric_arguments_are_validated() {
    let dir = scratch("args");
    put(&dir, "rc_op.sp", RC_OP);
    for (flag, val) in [
        ("--max-rss-mib", "0"),
        ("--max-rss-mib", "3"),
        ("--case-timeout", "0"),
        ("--reps", "0"),
        ("--reps", "100000"),
        ("--fuzz-count", "99999999"),
        ("--pin-cpu", "999"),
        ("--pin-cpu", "5000"),
    ] {
        let mut cmd = suite_cmd(&dir);
        cmd.args([flag, val]);
        let out = output_within(cmd, 60);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{flag} {val}: {err}");
        assert!(err.contains(flag), "{flag} {val}: {err}");
    }
}

/// ER-process-01: failure patterns were matched as substrings of every console line, including the echoed path of
/// the deck (`.../tran_idonly_error/ng_sparse.sp`).
#[test]
fn a_case_id_containing_error_is_not_an_ngspice_error() {
    if !ngspice_available() {
        eprintln!("SKIP error-in-id: ngspice not installed");
        return;
    }
    let cases = scratch("errid");
    put(
        &cases,
        "tran_idonly_error.sp",
        "rc with error in the id
* bench.id: tran_idonly_error
* bench.group: accuracy
V1 in 0 PULSE(0 1 0 1n 1n 1u 2u)
R1 in out 1k
C1 out 0 1n
.tran 10n 1u
.end
",
    );
    let (out, json, _md, _art) = run_suite(&cases, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = load(&json);
    let c = case(&v, "tran_idonly_error");
    assert_eq!(c["status"], "pass", "{c:#?}");
    assert_eq!(c["ngspice_status"], "ok");
}

/// ER-process-12: ngspice read `~/.spiceinit` (and honoured `SPICE_*` variables). A `.spiceinit` that says `quit`
/// made ngspice exit 0 without a rawfile for every deck.
#[test]
fn the_users_ngspice_startup_file_and_environment_do_not_leak_in() {
    if !ngspice_available() {
        eprintln!("SKIP spiceinit: ngspice not installed");
        return;
    }
    let dir = scratch("spiceinit");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "rc_op.sp", RC_OP);
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".spiceinit"), "quit\n").unwrap();
    // also in the deck's own directory, where ngspice looks for it next
    std::fs::write(cases.join(".spiceinit"), "quit\n").unwrap();
    let mut cmd = suite_cmd(&cases);
    cmd.env("HOME", &home)
        .env("SPICE_ASCIIRAWFILE", "1")
        .arg("--json")
        .arg(dir.join("out.json"));
    let out = output_within(cmd, 120);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    let v = load(&dir.join("out.json"));
    assert_eq!(
        case(&v, "rc_op")["status"],
        "pass",
        "{:#?}",
        case(&v, "rc_op")
    );
}

/// ER-process-09: a harness started with SIGCHLD ignored (some CI runners and `nohup`-style wrappers) made every
/// `wait4` fail with ECHILD, so every run was reported as a failure of its child.
#[test]
fn an_inherited_ignored_sigchld_does_not_break_the_runs() {
    if !ngspice_available() {
        eprintln!("SKIP sigchld: ngspice not installed");
        return;
    }
    let cases = scratch("sigchld");
    put(&cases, "rc_op.sp", RC_OP);
    let mut cmd = suite_cmd(&cases);
    unsafe {
        cmd.pre_exec(|| {
            libc::signal(libc::SIGCHLD, libc::SIG_IGN);
            Ok(())
        });
    }
    cmd.arg("--json").arg(cases.join("out.json"));
    let out = output_within(cmd, 120);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    let v = load(&cases.join("out.json"));
    assert_eq!(
        case(&v, "rc_op")["status"],
        "pass",
        "{:#?}",
        case(&v, "rc_op")
    );
}

/// Start a harness whose ngspice is a script that records its pid and then sleeps; returns the harness, the pid of
/// the ngspice stand-in and the scratch directory of the harness.
fn start_blocked_harness(name: &str) -> (std::process::Child, i32, Vec<PathBuf>, PathBuf) {
    let dir = scratch(name);
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(&cases, "rc_op.sp", RC_OP);
    let pidfile = dir.join("ng.pid");
    let ng = fake_ngspice(
        &dir,
        "ng_sleep",
        &format!("echo $$ > '{}'\nexec sleep 120", pidfile.display()),
    );
    let mut cmd = suite_cmd(&cases);
    cmd.arg("--ngspice")
        .arg(&ng)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let child = cmd.spawn().unwrap();
    let pid: i32 = wait_for_file(&pidfile, 60).trim().parse().unwrap();
    let scratches = scratch_dirs_of(child.id());
    (child, pid, scratches, dir)
}

/// ER-driver-10 and ER-process-04: on SIGTERM (`timeout`, a CI kill) the children kept running and the RAM-backed
/// scratch directory stayed behind. Now: children killed, scratch removed, exit 128 + signal.
#[test]
fn sigterm_kills_the_children_removes_the_scratch_directory_and_exits_143() {
    let (child, ng_pid, scratches, _dir) = start_blocked_harness("sigterm");
    assert!(
        !scratches.is_empty(),
        "the harness must have a scratch directory while it runs"
    );
    assert!(!process_gone(ng_pid));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let out = {
        let start = Instant::now();
        let mut child = child;
        loop {
            if child.try_wait().unwrap().is_some() {
                break child.wait_with_output().unwrap();
            }
            if start.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                panic!("the harness ignored SIGTERM");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(143), "{err}");
    assert!(err.contains("stopped by signal 15"), "{err}");
    assert!(
        wait_until(10, || process_gone(ng_pid)),
        "ngspice child {ng_pid} is still alive"
    );
    for s in &scratches {
        assert!(
            !s.exists(),
            "scratch directory {} was left behind",
            s.display()
        );
    }
}

/// A SIGKILL cannot be handled: the children that are not timed repetitions die with their parent (PDEATHSIG).
#[test]
fn a_killed_harness_does_not_leave_its_children_running() {
    let (mut child, ng_pid, scratches, _dir) = start_blocked_harness("sigkill");
    assert!(!process_gone(ng_pid));
    child.kill().unwrap();
    let _ = child.wait();
    let gone = wait_until(10, || process_gone(ng_pid));
    // whatever happened, do not leak the sleeper or the scratch directory of this test
    unsafe { libc::kill(ng_pid, libc::SIGKILL) };
    for s in scratches {
        let _ = std::fs::remove_dir_all(s);
    }
    assert!(
        gone,
        "ngspice child {ng_pid} survived the death of the harness"
    );
}

/// A RustSpice that cannot write its rawfile (disk full, missing directory) is a harness-side I/O failure: exit code
/// 74, a message that says so, no partial file; the suite then calls the case `unverified`, not a loud refusal.
#[test]
fn a_rawfile_that_cannot_be_written_exits_74_and_the_suite_calls_it_unverified() {
    let dir = scratch("rawfail");
    put(&dir, "ok.sp", RC_OP);
    let bad = dir.join("no_such_dir").join("ok.raw");
    let stats = dir.join("ok.json");
    let out = Command::new(BIN)
        .arg("run")
        .arg(dir.join("ok.sp"))
        .arg("--raw")
        .arg(&bad)
        .arg("--stats")
        .arg(&stats)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(74), "{err}");
    assert!(err.contains("Error: cannot write rawfile"), "{err}");
    // the simulation itself succeeded and printed its result
    assert!(String::from_utf8_lossy(&out.stdout).contains("[DC OP] Converged"));
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
    assert!(
        s["status"]
            .as_str()
            .unwrap()
            .starts_with("error: cannot write rawfile"),
        "{s}"
    );
    // a refusal keeps its own exit code 1
    put(&dir, "bad.sp", "bad\nR1 a 0 1k\nR2 a b\n.op\n.end\n");
    let out = Command::new(BIN)
        .arg("run")
        .arg(dir.join("bad.sp"))
        .arg("--raw")
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

/// ER-driver-19: a deck that selects KLU itself ran `ngspice_sparse` as KLU, so the headline compared KLU with KLU.
#[test]
fn a_deck_that_selects_klu_itself_is_timed_against_ngspice_klu_only() {
    if !ngspice_available() {
        eprintln!("SKIP klu deck: ngspice not installed");
        return;
    }
    let cases = scratch("klu_deck");
    put(
        &cases,
        "klu_deck.sp",
        &PERF_LADDER
            .replace("perf_ladder", "klu_deck")
            .replace(
                "* bench.size: 51",
                "* bench.size: 51\n* bench.ng.solver: both",
            )
            .replace(".op\n", ".options klu\n.op\n"),
    );
    let (out, json, _md, _art) = run_suite(&cases, &["--reps", "5"]);
    let _ = out;
    // the case is only comparable when this ngspice has KLU; the contender names are what matters
    let v = load(&json);
    let c = case(&v, "klu_deck");
    if let Some(t) = c["timing"].as_object() {
        let names: Vec<&str> = t["contenders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&"ngspice_sparse"), "{names:?}");
        assert!(names.contains(&"ngspice_klu"), "{names:?}");
    }
    assert!(
        c["notes"]
            .to_string()
            .contains("selects the KLU solver itself"),
        "{:#?}",
        c["notes"]
    );
}

/// `host.pin_cpu` is only reported when the pinning was applied; the report says so either way.
#[test]
fn the_pin_status_is_reported_honestly() {
    if !ngspice_available() {
        eprintln!("SKIP pin status: ngspice not installed");
        return;
    }
    let Some(cpu) = some_allowed_cpu() else {
        eprintln!("SKIP pin status: no readable Cpus_allowed_list");
        return;
    };
    let cases = scratch("pinstatus");
    put(&cases, "rc_op.sp", RC_OP);
    let (_out, json, md, _art) = run_suite(&cases, &["--pin-cpu", &cpu.to_string()]);
    let v = load(&json);
    assert_eq!(v["host"]["pin_cpu"], cpu);
    assert_eq!(v["host"]["pin_status"], "applied");
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.contains(&format!("CPU {cpu}")), "{text}");
    let (_out, json, _md, _art) = run_suite(&cases, &[]);
    let v = load(&json);
    assert!(v["host"]["pin_cpu"].is_null());
    assert_eq!(v["host"]["pin_status"], "not_requested");
}

/// ER-driver-21: the timed repetitions ignored the deck's `ngspice_error_ok` tag, so a deck whose (recoverable)
/// ngspice log lines look like errors was a normal run in the correctness step and a failed sample in every timed
/// repetition: the timing became `unstable` and the speedup disappeared without a word.
#[test]
fn timed_repetitions_honour_the_ngspice_error_ok_tag() {
    if !ngspice_available() {
        eprintln!("SKIP error_ok timing: ngspice not installed");
        return;
    }
    let dir = scratch("errok_timing");
    let cases = dir.join("cases");
    std::fs::create_dir_all(&cases).unwrap();
    put(
        &cases,
        "err_ok_case.sp",
        &PERF_LADDER.replace("perf_ladder", "err_ok_case").replace(
            "* bench.size: 51",
            "* bench.size: 51\n* bench.tags: ngspice_error_ok",
        ),
    );
    // the real ngspice behind a wrapper that adds an error-looking line to the console of this one case
    let ng = fake_ngspice(
        &dir,
        "ng_noisy",
        "case \"$4\" in */err_ok_case/*) echo 'Error: a recoverable notice that looks like a failure';; esac\nexec ngspice \"$@\"",
    );
    // the stand-in answers `--version` itself: hand it over to the real one
    let wrapper = std::fs::read_to_string(&ng).unwrap().replace(
        "--version) echo '** ngspice-99 : fake'; exit 0;;",
        "--version) exec ngspice --version;;",
    );
    std::fs::write(&ng, wrapper).unwrap();
    let (out, json, _md, _art) =
        run_suite(&cases, &["--ngspice", ng.to_str().unwrap(), "--reps", "5"]);
    let err = String::from_utf8_lossy(&out.stderr);
    let v = load(&json);
    let c = case(&v, "err_ok_case");
    assert_eq!(c["status"], "pass", "{err}\n{c:#?}");
    assert_eq!(c["timing_status"], "measured", "{c:#?}");
    assert_eq!(c["timing"]["reps_done"], 5, "{c:#?}");
}
