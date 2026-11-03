//! `rustspice run --raw`: the rawfile is written while the analyses run (`.tran` rows straight from the integrator)
//! and holds the same plots, bit for bit, as the in-memory result; its failure semantics are those of a rawfile written
//! at the end: a failed analysis leaves neither the rawfile nor `<raw>.raw.partial`, and an output error (missing
//! directory, full disk) lets the simulation finish, then exits with code 74 and leaves no file either.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rustspice_core::netlist::AnalysisCmd;
use rustspice_solver::{Simulator, TranSpec};
use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_rustspice");

fn scratch(name: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("raw_stream_{name}"));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

/// `.op`, `.tran` and `.dc` in one deck, with a diode series resistance (a device-internal node that is not written).
const MULTI: &str = "Operating point, transient and sweep
V1 in 0 PULSE(0 1 0.2n 0.1n 0.1n 1n 2.5n)
R1 in a 1k
C1 a 0 0.2p
R2 a out 2k
C2 out 0 0.3p
D1 out 0 DM
.model DM D(IS=1e-14 N=1.1 RS=20)
.op
.tran 20p 5n
.dc V1 0 1 0.25
.end
";

/// A linear RC whose transient overflows (negative resistance) at about t = 0.69 us: the analysis fails after
/// hundreds of rows were streamed.
const BLOWUP: &str = "Negative resistance blow-up
I1 0 a PULSE(0 1u 1n 1p 1p 1n 100n)
C1 a 0 1p
R1 a 0 -1k
.op
.tran 1n 2u
.end
";

fn run(deck: &Path, raw: Option<&Path>, extra: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.arg("run").arg(deck);
    if let Some(r) = raw {
        cmd.arg("--raw").arg(r);
    }
    cmd.args(extra).output().unwrap()
}

/// One plot of a binary rawfile: plot name, variable names, the `No. Points:` text as written, rows.
struct RawPlot {
    plotname: String,
    vars: Vec<String>,
    points_text: String,
    rows: Vec<Vec<f64>>,
}

fn parse_raw(bytes: &[u8]) -> Vec<RawPlot> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let end = pos
            + bytes[pos..]
                .windows(8)
                .position(|w| w == b"Binary:\n")
                .expect("Binary: marker")
            + 8;
        let header = std::str::from_utf8(&bytes[pos..end]).unwrap();
        let field = |key: &str| {
            header
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap_or_else(|| panic!("{key} in {header}"))
                .to_string()
        };
        let points_text = field("No. Points: ");
        let np: usize = points_text.trim().parse().unwrap();
        let vars: Vec<String> = header
            .lines()
            .filter(|l| l.starts_with('\t'))
            .map(|l| l.split('\t').nth(2).unwrap().to_string())
            .collect();
        let nv = vars.len();
        assert_eq!(nv, field("No. Variables: ").parse::<usize>().unwrap());
        let data = &bytes[end..end + np * nv * 8];
        let rows = data
            .chunks(nv * 8)
            .map(|r| {
                r.chunks(8)
                    .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            })
            .collect();
        out.push(RawPlot {
            plotname: field("Plotname: "),
            vars,
            points_text,
            rows,
        });
        pos = end + np * nv * 8;
    }
    out
}

/// The in-memory transient of `deck` (the library's collecting API), as rows of the rawfile's variables.
fn in_memory_tran(deck: &str, internal_points: bool, vars: &[String]) -> Vec<Vec<f64>> {
    let mut sim = Simulator::from_spice(deck).unwrap();
    sim.transient_solver.options.internal_points = internal_points;
    let spec = sim
        .netlist
        .analyses
        .iter()
        .find_map(|a| match *a {
            AnalysisCmd::Tran {
                tstep,
                tstop,
                tstart,
                tmax,
                uic,
            } => Some(TranSpec {
                tstep,
                tstop,
                tstart,
                tmax,
                uic,
            }),
            _ => None,
        })
        .unwrap();
    let res = sim.run_transient_spec(&spec).unwrap();
    let col = |name: &str| -> usize {
        if let Some(n) = name.strip_prefix("v(").and_then(|s| s.strip_suffix(')')) {
            return sim.netlist.find_node(n).unwrap() - 1;
        }
        let v = name
            .strip_prefix("i(")
            .and_then(|s| s.strip_suffix(')'))
            .unwrap();
        let soa = &sim.topology.soa;
        let k = soa
            .vsrc_names
            .iter()
            .position(|s| s.eq_ignore_ascii_case(v))
            .unwrap();
        soa.vsrc_branch_idx[k]
    };
    let cols: Vec<usize> = vars[1..].iter().map(|v| col(v)).collect();
    res.time_points
        .iter()
        .zip(&res.waveforms)
        .map(|(t, x)| {
            std::iter::once(*t)
                .chain(cols.iter().map(|&c| x[c]))
                .collect()
        })
        .collect()
}

fn bits(rows: &[Vec<f64>]) -> Vec<Vec<u64>> {
    rows.iter()
        .map(|r| r.iter().map(|x| x.to_bits()).collect())
        .collect()
}

fn partial(raw: &Path) -> PathBuf {
    raw.with_extension("raw.partial")
}

#[test]
fn the_streamed_rawfile_holds_every_plot_in_order_bit_for_bit() {
    let dir = scratch("multi");
    let deck = dir.join("multi.sp");
    std::fs::write(&deck, MULTI).unwrap();
    let raw = dir.join("multi.raw");
    let out = run(&deck, Some(&raw), &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!partial(&raw).exists());
    let plots = parse_raw(&std::fs::read(&raw).unwrap());
    let names: Vec<&str> = plots.iter().map(|p| p.plotname.as_str()).collect();
    assert_eq!(
        names,
        [
            "Operating Point",
            "Transient Analysis",
            "DC transfer characteristic"
        ]
    );
    let tran = &plots[1];
    // the diode's internal node is not written
    assert_eq!(tran.vars, ["time", "v(in)", "v(a)", "v(out)", "i(v1)"]);
    assert_eq!(tran.points_text, "251", "an exact row count, no padding");
    assert_eq!(tran.rows.len(), 251);
    assert_eq!(
        bits(&tran.rows),
        bits(&in_memory_tran(MULTI, false, &tran.vars))
    );
    assert_eq!(plots[2].rows.len(), 5);
}

#[test]
fn internal_points_fill_in_the_row_count_after_the_rows() {
    let dir = scratch("internal");
    let deck = dir.join("multi.sp");
    std::fs::write(&deck, MULTI).unwrap();
    let raw = dir.join("multi.raw");
    let stats = dir.join("multi.json");
    let out = run(
        &deck,
        Some(&raw),
        &["--internal-points", "--stats", stats.to_str().unwrap()],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let plots = parse_raw(&std::fs::read(&raw).unwrap());
    assert_eq!(
        plots.len(),
        3,
        "the plot after the patched header is intact"
    );
    let tran = &plots[1];
    // ngspice-style: the count is written into a blank field once known, padded with spaces
    assert_eq!(tran.points_text.len(), 20, "`{}`", tran.points_text);
    let n: usize = tran.points_text.trim_end().parse().unwrap();
    assert_eq!(n, tran.rows.len());
    assert!(n > 251, "internal steps were written ({n} rows)");
    assert_eq!(
        bits(&tran.rows),
        bits(&in_memory_tran(MULTI, true, &tran.vars))
    );
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
    assert_eq!(s["analyses"][1]["points"], n as u64);
    assert!(s["write_us"].as_f64().unwrap() > 0.0);
}

#[test]
fn a_transient_that_fails_after_streaming_rows_leaves_no_file() {
    let dir = scratch("fail");
    let deck = dir.join("blowup.sp");
    std::fs::write(&deck, BLOWUP).unwrap();
    let raw = dir.join("blowup.raw");
    // stale files of an earlier run go as well
    std::fs::write(&raw, b"stale").unwrap();
    std::fs::write(partial(&raw), b"stale").unwrap();
    let stats = dir.join("blowup.json");
    let out = run(&deck, Some(&raw), &["--stats", stats.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("Transient"), "{err}");
    assert!(!raw.exists(), "no rawfile after a failed analysis");
    assert!(
        !partial(&raw).exists(),
        "no partial rawfile after a failed analysis"
    );
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
    assert!(s["status"]
        .as_str()
        .unwrap()
        .starts_with("error: Transient"));
}

fn assert_harness_io_failure(out: &Output, raw: &Path, stats: &Path, tran_rows: usize) {
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(74), "{err}");
    assert!(err.contains("Error: cannot write rawfile"), "{err}");
    // the simulation itself ran to the end and printed its results
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[DC OP] Converged"), "{stdout}");
    assert!(
        stdout.contains(&format!("[Transient] {tran_rows} time points")),
        "{stdout}"
    );
    assert!(stdout.contains("[DC Sweep] 5 points"), "{stdout}");
    assert!(!raw.exists() && !partial(raw).exists());
    let s: Value = serde_json::from_str(&std::fs::read_to_string(stats).unwrap()).unwrap();
    assert!(
        s["status"]
            .as_str()
            .unwrap()
            .starts_with("error: cannot write rawfile"),
        "{s}"
    );
    assert_eq!(s["analysis"], "op+tran+dc");
}

#[test]
fn an_unwritable_rawfile_lets_the_run_finish_then_exits_74() {
    let dir = scratch("nodir");
    let deck = dir.join("multi.sp");
    std::fs::write(&deck, MULTI).unwrap();
    let raw = dir.join("no_such_dir").join("multi.raw");
    let stats = dir.join("multi.json");
    let out = run(&deck, Some(&raw), &["--stats", stats.to_str().unwrap()]);
    assert_harness_io_failure(&out, &raw, &stats, 251);
}

#[test]
fn a_rawfile_that_fills_up_mid_transient_exits_74_and_leaves_no_partial_file() {
    let dir = scratch("fsize");
    let deck = dir.join("multi.sp");
    // 5001 rows of 5 variables: 200 KB, more than the write buffer, so the rows reach the file during the analysis
    std::fs::write(&deck, MULTI.replace(".tran 20p 5n", ".tran 1p 5n")).unwrap();
    let raw = dir.join("multi.raw");
    let stats = dir.join("multi.json");
    // a 16 KiB file size limit fails a write in the middle of the transient (EFBIG, with SIGXFSZ ignored so that the
    // write returns the error instead of killing the process)
    let mut cmd = Command::new(BIN);
    cmd.arg("run")
        .arg(&deck)
        .arg("--raw")
        .arg(&raw)
        .arg("--stats")
        .arg(&stats);
    unsafe {
        cmd.pre_exec(|| {
            let lim = libc::rlimit {
                rlim_cur: 16 << 10,
                rlim_max: 16 << 10,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &lim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            Ok(())
        });
    }
    let out = cmd.output().unwrap();
    assert_harness_io_failure(&out, &raw, &stats, 5001);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("File too large"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Without `--raw` the rows go nowhere: the peak memory does not grow with the length of the transient (storing the
/// rows of the longer run would take 60 000 rows x 42 unknowns x 8 bytes = 19 MiB more).
#[test]
fn without_a_rawfile_memory_does_not_grow_with_the_number_of_rows() {
    let dir = scratch("noraw");
    let peak = |rows: usize| -> u64 {
        let mut deck = String::from("RC ladder\nV1 n0 0 PULSE(0 1 1n 1n 1n 20n 40n)\n");
        for k in 0..40 {
            deck.push_str(&format!("R{k} n{k} n{} 100\nC{k} n{} 0 1p\n", k + 1, k + 1));
        }
        deck.push_str(&format!(".tran 10p {}p\n.end\n", 10 * rows));
        let path = dir.join(format!("ladder_{rows}.sp"));
        std::fs::write(&path, deck).unwrap();
        let stats = dir.join(format!("ladder_{rows}.json"));
        let out = run(&path, None, &["--stats", stats.to_str().unwrap()]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains(&format!("[Transient] {} time points", rows + 1)),
            "{stdout}"
        );
        let s: Value = serde_json::from_str(&std::fs::read_to_string(&stats).unwrap()).unwrap();
        s["peak_rss_kib"].as_u64().unwrap()
    };
    let (short, long) = (peak(20_000), peak(80_000));
    assert!(
        long < short + 4 * 1024,
        "peak RSS {short} KiB for 20 000 rows, {long} KiB for 80 000 rows"
    );
}
