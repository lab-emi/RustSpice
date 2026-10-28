//! Honest re-implementation of the six legacy `rustspice bench-ngspice` benchmarks.
//!
//! The legacy harness compared a warm in-process RustSpice call against a cold ngspice subprocess, used a 1 ms
//! quantised `rusage` time as ngspice "solver time", labelled the default SPARSE solver "KLU", hard-coded its accuracy
//! columns and multiplied single ngspice runs by 400 / 128 for the sensitivity and batch benchmarks. This module keeps
//! the command (`bench-ngspice --output docs/benchmark_report.md`) and the six circuit families but measures every
//! number in the run that publishes it:
//!
//! * both contenders are black-box subprocesses writing a rawfile to tmpfs (`rustspice run --raw/--stats`,
//!   `ngspice -b -r`); repetitions are interleaved after a warm-up; min / median / stddev / noisy flag are recorded;
//! * end-to-end wall time (the headline), analysis-only time (ngspice `Total analysis time` vs RustSpice
//!   `analysis_us`, which EXCLUDES RustSpice's symbolic analysis / JIT setup booked as `compile_us`), the like-for-like
//!   analysis time (ngspice `Total analysis time` vs RustSpice `compile_us + analysis_us`) and the parse + compile /
//!   start-up overhead are reported separately, against BOTH ngspice solvers (SPARSE and `.options klu`); the headline
//!   compares against the faster one per metric;
//! * accuracy is the full waveform / sweep against a tightly converged ngspice reference through the engine's
//!   `compare` machinery, with ngspice's own default-tolerance error vs the same reference and an independent oracle
//!   (closed-form ladder recursion, `oracle.rs`) where one exists; metrics that cannot be computed print `n/a`;
//! * the sensitivity benchmark (b5) compares the adjoint gradient of all parameters with ngspice `.sens`, with central
//!   finite differences run as one ngspice process (`.control` alter loop) and as 2P / P+1 process launches, and with
//!   a RustSpice finite-difference loop; the batch benchmark (b6) runs the SAME corners in ngspice sequentially, in
//!   parallel, in one `.control` loop and as one `.dc` sweep;
//! * every number is `null` / `n/a` plus a status string when it was not measured (never 0).
//!
//! Resource model (AGENTS.md Rule 9): the parent holds at most one tight b2 reference plot (10 041 rows x 2 003
//! columns x 8 B = 161 MB) next to the as-is and RustSpice plots; measured parent peak RSS is printed in the report
//! (< 1 GiB). Children run under a wall-clock limit and a 3 GiB RSS cap; the tight b2 reference is the largest child
//! (~0.4 GiB).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Args;
use serde::{Deserialize, Serialize};

use super::compare::{compare, CompareInput, CompareResult};
use super::meta::{self, Meta, NgSolver, TIGHT_OPTIONS, TRAN_REF_OPTIONS};
use super::ngspice::{self, NgClass};
use super::oracle;
use super::rawfile::{Plot, PlotKind};
use super::rustsim::{self, loadavg, run_guarded, ChildRun, Exit, Limits, RsClass, TmpDir};
use super::stats::{loglog_fit, summarize, time_at_equal_error, EqualError, Fit, Summary};

/// Arguments of `rustspice bench-ngspice`.
#[derive(Args, Debug, Clone)]
pub struct LegacyArgs {
    /// Path to write the Markdown benchmark report
    #[arg(long, default_value = "docs/benchmark_report.md")]
    pub output: PathBuf,
    /// Timed repetitions per contender (interleaved after one warm-up; a contender whose own warm-up takes over 1 s / 5 s
    /// is repeated at most 5 / 3 times, heavy multi-launch workloads at most 3; each cap applies only to that contender)
    #[arg(long, default_value_t = 7)]
    pub reps: usize,
    /// Also write the full results as JSON
    #[arg(long)]
    pub json: Option<PathBuf>,
    /// ngspice executable
    #[arg(long, default_value = "ngspice")]
    pub ngspice: String,
    /// Pin sequential child processes to this CPU (parallel workloads are never pinned)
    #[arg(long)]
    pub pin_cpu: Option<usize>,
    /// Skip all timing repetitions (accuracy only)
    #[arg(long)]
    pub no_timing: bool,
    /// Run only these benchmarks (comma separated: b1,b2,b3,b4,b5,b6)
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,
    /// Wall-clock limit per child process in seconds
    #[arg(long, default_value_t = 120)]
    pub case_timeout: u64,
    /// Internal: execute one in-process worker job (JSON job file) and exit
    #[arg(long, hide = true)]
    pub worker: Option<PathBuf>,
}

const SCHEMA: u32 = 1;
/// Series resistance of the ladder (ohm).
const R_SER: f64 = 10.0;
/// Shunt resistance of the legacy ladder (ohm).
const R_SH_LEGACY: f64 = 10_000.0;
/// Step policy as stated in the report (what each tool writes, and what the comparison does afterwards).
const STEP_POLICY: &str = "RustSpice: adaptive step control (local truncation error from divided differences, trapezoid with backward-Euler / BDF-2 restarts at waveform breakpoints, maximum step `min(tstep, tmax, (tstop - tstart) / 50)`), one rawfile row per nominal `k * tstep` point. ngspice: default adaptive step control with maximum step tstep; `ngspice -b -r` writes ITS OWN internal time points (not a uniform grid), so the row counts of the two tools differ (see the transient tables). The comparison interpolates RustSpice, ngspice and the reference linearly onto the nominal `k * tstep` grid afterwards; the ngspice error reported therefore depends on that comparison grid.";
/// Resident-memory cap per child (KiB).
const CHILD_RSS_CAP_KIB: u64 = 3 * 1024 * 1024;
/// Corner supply start / step of the legacy batch benchmark.
const CORNER_V0: f64 = 1.5;
const CORNER_DV: f64 = 0.03;
/// Relative step of the central finite differences.
const FD_H_CENTRAL: f64 = 1e-4;
/// Relative step of the forward finite differences (P+1 launches).
const FD_H_FORWARD: f64 = 1e-6;
/// Largest RC line (stages) for which the `tran_lti` oracle converges within its work budget (observed: it gives up at
/// 250 stages and succeeds at 120).
const TRAN_ORACLE_MAX_STAGES: usize = 120;
/// Swing floor for transient comparisons: 1 % of the 3.3 V supply (nodes that never move are `vacuous`).
const TRAN_FLOOR_V: f64 = 3.3e-2;

// ---------------------------------------------------------------------------------------------------------
// Previously published numbers (docs/benchmark_report.md and README.md at commit 7bb6d36)
// ---------------------------------------------------------------------------------------------------------

/// One row of the legacy table as it was published (never re-measured; quoted only for comparison).
#[derive(Clone, Debug, Serialize)]
pub struct Published {
    pub id: &'static str,
    pub name: &'static str,
    pub scale: &'static str,
    pub ng_wall_ms: f64,
    pub ng_solver_ms: f64,
    pub rs_total_ms: f64,
    pub rs_solver_ms: f64,
    pub max_dv: &'static str,
    pub e2e_speedup: f64,
    pub solver_speedup: f64,
}

pub const PUBLISHED: [Published; 6] = [
    Published {
        id: "b1",
        name: "5,000-Node Power Grid (DC OP)",
        scale: "10,000 Resistors, 5,002 MNA variables",
        ng_wall_ms: 58.47,
        ng_solver_ms: 50.00,
        rs_total_ms: 11.053,
        rs_solver_ms: 0.206,
        max_dv: "2.16e-45",
        e2e_speedup: 5.3,
        solver_speedup: 243.2,
    },
    Published {
        id: "b2",
        name: "2,000-Node RC Line (500-Step Tran)",
        scale: "4,000 R/C elements, 500 time steps",
        ng_wall_ms: 244.81,
        ng_solver_ms: 238.00,
        rs_total_ms: 32.218,
        rs_solver_ms: 27.836,
        max_dv: "0.00e0",
        e2e_speedup: 7.6,
        solver_speedup: 8.6,
    },
    Published {
        id: "b3",
        name: "64-Diode Nonlinear Array (201-Pt DC Sweep)",
        scale: "64 Diodes + 128 Resistors, 201 sweep points",
        ng_wall_ms: 8.71,
        ng_solver_ms: 5.00,
        rs_total_ms: 2.898,
        rs_solver_ms: 2.664,
        max_dv: "2.15e-14",
        e2e_speedup: 3.0,
        solver_speedup: 1.9,
    },
    Published {
        id: "b4",
        name: "100-Stage CMOS Inverter Chain (Tran)",
        scale: "200 NMOS/PMOS + 100 Caps, 400 time steps",
        ng_wall_ms: 42.14,
        ng_solver_ms: 39.00,
        rs_total_ms: 16.265,
        rs_solver_ms: 15.857,
        max_dv: "3.31e-12",
        e2e_speedup: 2.6,
        solver_speedup: 2.5,
    },
    Published {
        id: "b5",
        name: "200-Param Sensitivity (Adjoint vs. FD)",
        scale: "200 Resistor Gradients (1 Adjoint Solve vs. 400 FD)",
        ng_wall_ms: 2167.17,
        ng_solver_ms: 800.00,
        rs_total_ms: 0.345,
        rs_solver_ms: 0.081,
        max_dv: "1.50e-11",
        e2e_speedup: 6284.9,
        solver_speedup: 9867.0,
    },
    Published {
        id: "b6",
        name: "128-Corner PVT Batch (Rayon Parallel)",
        scale: "128 Nonlinear 32-Diode Corners in Parallel",
        ng_wall_ms: 686.08,
        ng_solver_ms: 256.00,
        rs_total_ms: 2.136,
        rs_solver_ms: 1.940,
        max_dv: "0.00e0",
        e2e_speedup: 321.1,
        solver_speedup: 131.9,
    },
];

// ---------------------------------------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------------------------------------

/// SPICE-safe, exactly round-tripping decimal text of a float (`10.0`, `1e-5`, `62500000.0`).
pub fn num(x: f64) -> String {
    format!("{x:?}")
}

/// Time in microseconds as `412 µs`, `3.21 ms`, `1.52 s`; non-finite values print `n/a`.
pub fn fmt_time(us: f64) -> String {
    if !us.is_finite() {
        return "n/a".to_string();
    }
    if us < 1.0 {
        format!("{us:.2} µs")
    } else if us < 100.0 {
        format!("{us:.1} µs")
    } else if us < 1000.0 {
        format!("{us:.0} µs")
    } else if us < 100_000.0 {
        format!("{:.2} ms", us / 1e3)
    } else if us < 1e6 {
        format!("{:.0} ms", us / 1e3)
    } else {
        format!("{:.2} s", us / 1e6)
    }
}

pub fn fmt_opt_time(us: Option<f64>) -> String {
    us.map_or("n/a".to_string(), fmt_time)
}

/// Speed ratio as `5.3x`; `None` prints `n/a`.
pub fn fmt_x(v: Option<f64>) -> String {
    match v {
        Some(x) if !x.is_finite() => "n/a".to_string(),
        Some(x) if x >= 100.0 => format!("{x:.0}x"),
        Some(x) if x >= 10.0 => format!("{x:.1}x"),
        Some(x) => format!("{x:.2}x"),
        None => "n/a".to_string(),
    }
}

/// Scientific notation with three significant digits; `None` prints `n/a`.
pub fn fmt_sci(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => {
            if x == 0.0 {
                "0 (identical)".to_string()
            } else {
                format!("{x:.2e}")
            }
        }
        _ => "n/a".to_string(),
    }
}

pub fn fmt_pct(v: Option<f64>) -> String {
    v.filter(|x| x.is_finite())
        .map_or("n/a".to_string(), |x| format!("{:.2} %", 100.0 * x))
}

/// Median of a summary as a time cell.
fn med(s: &Option<Summary>) -> String {
    s.as_ref().map_or("n/a".to_string(), |s| fmt_time(s.median))
}

/// `median (min)` cell.
fn med_min(s: &Option<Summary>) -> String {
    s.as_ref().map_or("n/a".to_string(), |s| {
        format!("{} ({})", fmt_time(s.median), fmt_time(s.min))
    })
}

fn fmt_mib(kib: Option<f64>) -> String {
    kib.map_or("n/a".to_string(), |k| format!("{:.1}", k / 1024.0))
}

fn fmt_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

// ---------------------------------------------------------------------------------------------------------
// Deck builders (identical text is fed to both simulators; no `.control` block except where stated)
// ---------------------------------------------------------------------------------------------------------

/// Resistors of the ladder in deck order: `(name, node a, node b, ohm)`; node 0 is ground.
pub fn ladder_resistors(stages: usize, rsh: f64) -> Vec<(String, usize, usize, f64)> {
    let mut res = Vec::with_capacity(2 * stages);
    for k in 1..=stages {
        res.push((format!("Rser{k}"), k, k + 1, R_SER));
        res.push((format!("Rsh{k}"), k + 1, 0, rsh));
    }
    res
}

/// Ladder deck with `tail` (analysis / control text) before `.end`; `override_` replaces the value of the resistor
/// at that deck-order index (used by the finite-difference launches).
pub fn ladder_deck_with(
    stages: usize,
    rsh: f64,
    tail: &str,
    override_: Option<(usize, f64)>,
) -> String {
    let mut s = String::with_capacity(stages * 44 + tail.len() + 96);
    s.push_str("Large Resistor Ladder Power Grid Benchmark\nV1 1 0 DC 5.0\n");
    for (i, (name, a, b, r)) in ladder_resistors(stages, rsh).into_iter().enumerate() {
        let r = match override_ {
            Some((k, v)) if k == i => v,
            _ => r,
        };
        let _ = writeln!(s, "{name} {a} {b} {}", num(r));
    }
    s.push_str(tail);
    if !tail.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(".end\n");
    s
}

/// The legacy b1 deck: `stages` series / shunt pairs and an `.op` card.
pub fn ladder_deck(stages: usize, rsh: f64) -> String {
    ladder_deck_with(stages, rsh, ".op", None)
}

/// Shunt resistance that makes the ladder voltage decay by `exp(-depth)` over its length, so that every node carries
/// a non-vacuous O(1) voltage (the legacy 10 kohm shunt leaves every node beyond ~500 below 1 uV).
pub fn shunt_for_depth(stages: usize, depth: f64) -> f64 {
    R_SER * (stages as f64 / depth).powi(2)
}

/// Distributed RC line deck (legacy b2): 5 ohm / 10 fF per stage, 3.3 V pulse, `.tran <tstep> 5n`.
pub fn rc_line_deck(stages: usize, tstep: &str) -> String {
    let mut s = String::with_capacity(stages * 50 + 128);
    s.push_str("Distributed RC Transmission Line Transient Benchmark\n");
    s.push_str("V1 1 0 PULSE(0.0 3.3 0.0 10p 10p 10n 20n)\n");
    for k in 1..=stages {
        let _ = writeln!(s, "R{k} {k} {} 5.0", k + 1);
        let _ = writeln!(s, "C{k} {} 0 10f", k + 1);
    }
    let _ = writeln!(s, ".tran {tstep} 5n");
    s.push_str(".end\n");
    s
}

/// Diode cascade deck (legacy b3 / b6): supply `v1`, analysis / control text `tail`.
pub fn diode_cascade_deck_with(n: usize, v1: f64, tail: &str) -> String {
    let mut s = String::with_capacity(n * 60 + tail.len() + 128);
    s.push_str("Nonlinear PN Diode Cascade DC Sweep Benchmark\n");
    s.push_str(".model DMOD D(IS=1e-14 N=1.05)\n");
    let _ = writeln!(s, "V1 1 0 DC {}", num(v1));
    for k in 1..=n {
        let n_in = if k == 1 { 1 } else { n + k };
        let n_mid = n + k + 1;
        let _ = writeln!(s, "R{k} 1 {n_mid} 1000.0");
        let _ = writeln!(s, "D{k} {n_mid} 0 DMOD");
        let _ = writeln!(s, "Rlink{k} {n_in} {n_mid} 10000.0");
    }
    s.push_str(tail);
    if !tail.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(".end\n");
    s
}

/// The legacy b3 deck: 201-point `.dc V1 0 5 0.025` sweep.
pub fn diode_cascade_deck(n: usize) -> String {
    diode_cascade_deck_with(n, 5.0, ".dc V1 0.0 5.0 0.025")
}

/// CMOS inverter chain deck (legacy b4): level-1 devices, 20 fF loads, `.tran 5p 2n`.
pub fn cmos_chain_deck(stages: usize) -> String {
    let mut s = String::with_capacity(stages * 100 + 256);
    s.push_str("CMOS Inverter Chain Transient & DC Benchmark\n");
    s.push_str(".model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)\n");
    s.push_str(".model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)\n");
    s.push_str("VDD vdd 0 DC 3.3\n");
    s.push_str("VIN in1 0 PULSE(0.0 3.3 0.2n 0.1n 0.1n 2n 4n)\n");
    for k in 1..=stages {
        let inp = if k == 1 {
            "in1".to_string()
        } else {
            format!("n{}", k - 1)
        };
        let out = format!("n{k}");
        let _ = writeln!(s, "MP{k} {out} {inp} vdd vdd PMOD W=20u L=1u");
        let _ = writeln!(s, "MN{k} {out} {inp} 0 0 NMOD W=10u L=1u");
        let _ = writeln!(s, "CL{k} {out} 0 20f");
    }
    s.push_str(".tran 5p 2n\n.end\n");
    s
}

/// Trivial deck for the process start-up floors.
pub fn floor_deck() -> String {
    "Start-up floor\nV1 1 0 DC 1.0\nR1 1 0 1000.0\n.op\n.end\n".to_string()
}

/// Corner supply voltages of the batch benchmark.
pub fn corner_values(n: usize) -> Vec<f64> {
    (0..n).map(|c| CORNER_V0 + CORNER_DV * c as f64).collect()
}

/// `.dc` card that visits exactly the corner values (stop nudged by 10 % of a step so that rounding cannot add or
/// drop the last point).
pub fn corner_sweep_card(n: usize) -> String {
    let stop = CORNER_V0 + CORNER_DV * (n as f64 - 1.0) + 0.1 * CORNER_DV;
    format!(".dc V1 {} {} {}", num(CORNER_V0), num(stop), num(CORNER_DV))
}

// ---------------------------------------------------------------------------------------------------------
// Independent references (no RustSpice and no ngspice code involved)
// ---------------------------------------------------------------------------------------------------------

/// Exact node voltages `v(1)..v(n+1)` of the uniform resistor ladder (backward impedance recursion in f64; error
/// ~1e-14 V at 5 000 stages, checked against the dense `oracle::evaluate("dc_linear")` in the unit tests).
pub fn ladder_voltages(n: usize, rser: f64, rsh: f64, vin: f64) -> Vec<f64> {
    // z[j] = impedance to ground seen at node j looking away from the source
    let mut z = vec![0.0; n + 2];
    z[n + 1] = rsh;
    for j in (2..=n).rev() {
        let series = rser + z[j + 1];
        z[j] = rsh * series / (rsh + series);
    }
    let mut v = Vec::with_capacity(n + 1);
    v.push(vin);
    for k in 1..=n {
        v.push(v[k - 1] * z[k + 1] / (rser + z[k + 1]));
    }
    v
}

/// Dense MNA model of the ladder used as the gradient reference.
#[derive(Clone, Debug)]
pub struct LadderNet {
    pub stages: usize,
    pub vin: f64,
    pub res: Vec<(String, usize, usize, f64)>,
}

pub fn ladder_net(stages: usize, rsh: f64) -> LadderNet {
    LadderNet {
        stages,
        vin: 5.0,
        res: ladder_resistors(stages, rsh),
    }
}

impl LadderNet {
    fn dim(&self) -> usize {
        self.stages + 2
    }

    /// Row-major MNA matrix; unknowns are `v(1)..v(n+1)` then `i(V1)`.
    fn matrix(&self, r: &[f64]) -> Vec<f64> {
        let dim = self.dim();
        let mut a = vec![0.0; dim * dim];
        for ((_, p, q, _), &ohm) in self.res.iter().zip(r) {
            let g = 1.0 / ohm;
            let idx = |node: usize| node.checked_sub(1);
            if let Some(i) = idx(*p) {
                a[i * dim + i] += g;
            }
            if let Some(j) = idx(*q) {
                a[j * dim + j] += g;
            }
            if let (Some(i), Some(j)) = (idx(*p), idx(*q)) {
                a[i * dim + j] -= g;
                a[j * dim + i] -= g;
            }
        }
        a[dim - 1] += 1.0; // row 0, branch column
        a[(dim - 1) * dim] += 1.0;
        a
    }

    fn rhs(&self) -> Vec<f64> {
        let mut b = vec![0.0; self.dim()];
        b[self.dim() - 1] = self.vin;
        b
    }

    fn nominal(&self) -> Vec<f64> {
        self.res.iter().map(|r| r.3).collect()
    }

    /// Solve the network for resistor values `r` (compensated-residual dense LU).
    pub fn solve(&self, r: &[f64]) -> Result<Vec<f64>, String> {
        oracle::solve_dense_refined(self.dim(), &self.matrix(r), &self.rhs())
    }

    /// `dv(target)/dR_k` for every resistor (deck order) from one dense forward and one dense transposed solve.
    pub fn gradients(&self, target_node: usize) -> Result<Vec<f64>, String> {
        let dim = self.dim();
        let r = self.nominal();
        let a = self.matrix(&r);
        let x = oracle::solve_dense_refined(dim, &a, &self.rhs())?;
        let mut at = vec![0.0; dim * dim];
        for i in 0..dim {
            for j in 0..dim {
                at[j * dim + i] = a[i * dim + j];
            }
        }
        let mut e = vec![0.0; dim];
        e[target_node - 1] = 1.0;
        let lam = oracle::solve_dense_refined(dim, &at, &e)?;
        let at_node = |v: &[f64], node: usize| node.checked_sub(1).map_or(0.0, |i| v[i]);
        Ok(self
            .res
            .iter()
            .map(|(_, p, q, ohm)| {
                (at_node(&lam, *p) - at_node(&lam, *q)) * (at_node(&x, *p) - at_node(&x, *q))
                    / (ohm * ohm)
            })
            .collect())
    }

    /// Voltage of `node` for resistor values `r` (finite-difference reference in the unit tests).
    pub fn voltage(&self, r: &[f64], node: usize) -> Result<f64, String> {
        Ok(self.solve(r)?[node - 1])
    }
}

/// Central difference.
pub fn central_diff(f_plus: f64, f_minus: f64, dx: f64) -> f64 {
    (f_plus - f_minus) / (2.0 * dx)
}

/// Richardson extrapolation of two central differences with steps `h` and `h/2` (error O(h^4)).
pub fn richardson(d_h: f64, d_half: f64) -> f64 {
    (4.0 * d_half - d_h) / 3.0
}

/// Accuracy of a gradient vector against the reference.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GradError {
    pub n: usize,
    /// max |g - ref|
    pub max_abs: f64,
    /// max |g - ref| / max |ref|
    pub max_over_scale: Option<f64>,
    /// max |g - ref| / |ref| over entries with |ref| >= 1e-3 max |ref|
    pub max_rel_significant: Option<f64>,
    pub worst_index: usize,
}

pub fn gradient_error(g: &[f64], reference: &[f64]) -> Result<GradError, String> {
    if g.len() != reference.len() || g.is_empty() {
        return Err(format!(
            "gradient length {} != reference length {}",
            g.len(),
            reference.len()
        ));
    }
    if g.iter().any(|x| !x.is_finite()) {
        return Err("gradient contains non-finite values".to_string());
    }
    let scale = reference.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let mut max_abs = 0.0f64;
    let mut worst = 0;
    let mut max_rel: Option<f64> = None;
    for (i, (a, r)) in g.iter().zip(reference).enumerate() {
        let d = (a - r).abs();
        if d >= max_abs {
            max_abs = d;
            worst = i;
        }
        if scale > 0.0 && r.abs() >= 1e-3 * scale {
            let rel = d / r.abs();
            max_rel = Some(max_rel.map_or(rel, |m: f64| m.max(rel)));
        }
    }
    Ok(GradError {
        n: g.len(),
        max_abs,
        max_over_scale: (scale > 0.0).then_some(max_abs / scale),
        max_rel_significant: max_rel,
        worst_index: worst,
    })
}

/// Error of a table of per-corner results against a reference table (aligned by label).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TableError {
    pub rows: usize,
    pub labels: usize,
    pub max_abs: f64,
    /// max |a - ref| / max(|ref|, 1e-6)
    pub max_rel: f64,
    pub worst_row: usize,
    pub worst_label: String,
}

pub fn table_error(
    labels: &[String],
    rows: &[Vec<f64>],
    ref_labels: &[String],
    ref_rows: &[Vec<f64>],
) -> Result<TableError, String> {
    if rows.len() != ref_rows.len() || rows.is_empty() {
        return Err(format!(
            "{} corner rows vs {} reference rows",
            rows.len(),
            ref_rows.len()
        ));
    }
    let map: Vec<(usize, usize)> = labels
        .iter()
        .enumerate()
        .filter_map(|(i, l)| ref_labels.iter().position(|r| r == l).map(|j| (i, j)))
        .collect();
    if map.is_empty() {
        return Err("no common vectors".to_string());
    }
    let mut max_abs = 0.0f64;
    let mut max_rel = 0.0f64;
    let (mut wr, mut wl) = (0usize, 0usize);
    for (r, (a, b)) in rows.iter().zip(ref_rows).enumerate() {
        for &(i, j) in &map {
            let (x, y) = (a[i], b[j]);
            if !x.is_finite() || !y.is_finite() {
                return Err(format!("non-finite value at row {r}, `{}`", labels[i]));
            }
            let d = (x - y).abs();
            max_rel = max_rel.max(d / y.abs().max(1e-6));
            if d >= max_abs {
                max_abs = d;
                wr = r;
                wl = i;
            }
        }
    }
    Ok(TableError {
        rows: rows.len(),
        labels: map.len(),
        max_abs,
        max_rel,
        worst_row: wr,
        worst_label: labels[wl].clone(),
    })
}

/// Values printed by ngspice as `<name> = <value>` (case-insensitive name), in order of appearance.
pub fn parse_prints(stdout: &str, name: &str) -> Vec<f64> {
    let prefix = format!("{} =", name.to_ascii_lowercase());
    stdout
        .lines()
        .filter_map(|l| {
            let t = l.trim().to_ascii_lowercase();
            t.strip_prefix(&prefix)
                .and_then(|rest| rest.trim().parse::<f64>().ok())
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Measurement machinery
// ---------------------------------------------------------------------------------------------------------

/// Result payload of the correctness (warm-up) run of a contender.
#[derive(Clone, Debug)]
pub enum Output {
    Plots(Vec<Plot>),
    /// Gradients in resistor deck order.
    Vector(Vec<f64>),
    /// One row per corner, one column per label (`v(node)`).
    Table {
        labels: Vec<String>,
        rows: Vec<Vec<f64>>,
    },
}

/// One timed execution of a contender (a whole workload, possibly many child processes).
#[derive(Clone, Debug, Default)]
pub struct Sample {
    pub e2e_us: f64,
    /// The tool's own analysis time: ngspice `Total analysis time` (includes its matrix setup) / RustSpice
    /// `analysis_us` (EXCLUDES RustSpice's `compile_us`).
    pub analysis_us: Option<f64>,
    pub parse_us: Option<f64>,
    pub compile_us: Option<f64>,
    pub write_us: Option<f64>,
    pub rss_kib: Option<u64>,
    /// Worker threads the workload really used, when the workload itself reports it (RustSpice worker jobs).
    pub threads: Option<usize>,
    pub info: Option<String>,
    pub output: Option<Output>,
}

type RunFn<'a> = Box<dyn FnMut(bool) -> Result<Sample, String> + 'a>;

/// A workload under test. `run(true)` is the warm-up / correctness run and must return the output payload.
pub struct Contender<'a> {
    pub name: String,
    pub note: String,
    pub launches: usize,
    pub threads: usize,
    /// Multi-launch workloads are repeated at most 3 times (each repetition already averages many launches).
    pub heavy: bool,
    pub run: RunFn<'a>,
}

#[derive(Clone, Debug)]
pub struct Measured {
    pub name: String,
    pub note: String,
    pub launches: usize,
    pub threads: usize,
    pub status: String,
    pub info: Option<String>,
    pub warm_e2e_us: Option<f64>,
    /// Timed repetitions this contender was scheduled for (0 when timing is off or the warm-up failed).
    pub reps_planned: usize,
    pub output: Option<Output>,
    pub samples: Vec<Sample>,
}

impl Measured {
    fn ok(&self) -> bool {
        self.status == "ok"
    }
}

/// Repetitions for a workload given ITS OWN warm-up run: slow workloads are repeated fewer times.
pub fn effective_reps(requested: usize, warmup_us: f64) -> usize {
    let r = requested.max(1);
    if warmup_us > 5e6 {
        r.min(3)
    } else if warmup_us > 1e6 {
        r.min(5)
    } else {
        r
    }
}

/// Repetitions of one contender: heavy multi-launch workloads use at most 3, and a slow warm-up caps only the
/// contender that was slow (a slow neighbour never lowers the repetitions of the contenders being compared).
pub fn contender_reps(requested: usize, heavy: bool, warmup_us: f64) -> usize {
    let class = if heavy { requested.min(3) } else { requested };
    effective_reps(class, warmup_us)
}

/// Order of the contenders in repetition `rep`: a rotation, so that no contender is always measured first.
pub fn rotation(n: usize, rep: usize) -> Vec<usize> {
    (0..n).map(|i| (i + rep) % n.max(1)).collect()
}

/// Warm-up (also the correctness run) then interleaved timed repetitions.
pub fn measure(cs: &mut [Contender], reps: usize, timing: bool) -> Vec<Measured> {
    let mut out: Vec<Measured> = cs
        .iter()
        .map(|c| Measured {
            name: c.name.clone(),
            note: c.note.clone(),
            launches: c.launches,
            threads: c.threads,
            status: "ok".to_string(),
            info: None,
            warm_e2e_us: None,
            reps_planned: 0,
            output: None,
            samples: Vec::new(),
        })
        .collect();
    for (c, m) in cs.iter_mut().zip(out.iter_mut()) {
        match (c.run)(true) {
            Ok(mut s) => {
                m.warm_e2e_us = Some(s.e2e_us);
                m.info = s.info.take();
                m.output = s.output.take();
                // the thread count of a workload is what it reports, not what the harness assumed
                if let Some(t) = s.threads {
                    m.threads = t;
                }
            }
            Err(e) => m.status = format!("failed: {e}"),
        }
    }
    if !timing {
        return out;
    }
    // every contender is capped by ITS OWN warm-up (and heavy multi-launch workloads by 3)
    let limits: Vec<usize> = cs
        .iter()
        .zip(&out)
        .map(|(c, m)| match (m.ok(), m.warm_e2e_us) {
            (true, Some(w)) => contender_reps(reps, c.heavy, w),
            _ => 0,
        })
        .collect();
    for (m, l) in out.iter_mut().zip(&limits) {
        m.reps_planned = *l;
    }
    for rep in 0..limits.iter().copied().max().unwrap_or(0) {
        for i in rotation(cs.len(), rep) {
            if rep >= limits[i] || !out[i].ok() {
                continue;
            }
            match (cs[i].run)(false) {
                Ok(s) => out[i].samples.push(s),
                Err(e) => out[i].status = format!("failed in repetition {}: {e}", rep + 1),
            }
        }
    }
    out
}

/// Summary of one contender (all times in microseconds).
#[derive(Clone, Debug, Default, Serialize)]
pub struct ContenderStats {
    pub name: String,
    pub note: String,
    pub status: String,
    pub info: Option<String>,
    pub launches: usize,
    /// Worker threads (as reported by the workload itself where it reports them)
    pub threads: usize,
    /// Repetitions completed
    pub n: usize,
    /// Repetitions scheduled (0 when timing is off); `n < reps_planned` means a repetition failed
    pub reps_planned: usize,
    pub e2e_us: Option<Summary>,
    /// ngspice `Total analysis time` (which includes ngspice's own matrix setup) / RustSpice `analysis_us` (which
    /// EXCLUDES `compile_us`, RustSpice's symbolic analysis / JIT); null when the tool does not report it
    pub analysis_us: Option<Summary>,
    /// RustSpice `compile_us + analysis_us` per repetition: the like-for-like counterpart of ngspice's `Total analysis
    /// time`; null for tools that report no compile time
    pub compile_analysis_us: Option<Summary>,
    /// e2e - analysis per repetition (start-up, parse, setup, output)
    pub overhead_us: Option<Summary>,
    pub parse_us: Option<Summary>,
    pub compile_us: Option<Summary>,
    pub write_us: Option<Summary>,
    pub rss_kib: Option<Summary>,
}

impl ContenderStats {
    /// Completed every scheduled repetition without a failure.
    pub fn completed(&self) -> bool {
        self.status == "ok" && self.n > 0 && self.n == self.reps_planned
    }
}

fn summary_of(samples: &[Sample], f: impl Fn(&Sample) -> Option<f64>) -> Option<Summary> {
    let v: Vec<f64> = samples.iter().filter_map(f).collect();
    if v.len() == samples.len() {
        summarize(&v)
    } else {
        None
    }
}

pub fn stats_of(m: &Measured) -> ContenderStats {
    let s = &m.samples;
    ContenderStats {
        name: m.name.clone(),
        note: m.note.clone(),
        status: m.status.clone(),
        info: m.info.clone(),
        launches: m.launches,
        threads: m.threads,
        n: s.len(),
        reps_planned: m.reps_planned,
        e2e_us: summary_of(s, |x| Some(x.e2e_us)),
        analysis_us: summary_of(s, |x| x.analysis_us),
        compile_analysis_us: summary_of(s, |x| Some(x.analysis_us? + x.compile_us?)),
        overhead_us: summary_of(s, |x| x.analysis_us.map(|v| x.e2e_us - v)),
        parse_us: summary_of(s, |x| x.parse_us),
        compile_us: summary_of(s, |x| x.compile_us),
        write_us: summary_of(s, |x| x.write_us),
        rss_kib: summary_of(s, |x| x.rss_kib.map(|k| k as f64)),
    }
}

/// Speedup of RustSpice against each ngspice solver and against the faster one.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Ratio {
    pub vs_sparse: Option<f64>,
    pub vs_klu: Option<f64>,
    pub vs_fastest: Option<f64>,
    /// `sparse` or `klu`: the faster ngspice solver for this metric
    pub fastest: Option<String>,
}

/// `ng_*` / `rs` are the same statistic (median or min) of the same metric; ratios are `ng / rs` (>1: RustSpice faster).
pub fn ratio(ng_sparse: Option<f64>, ng_klu: Option<f64>, rs: Option<f64>) -> Ratio {
    let ok = |x: Option<f64>| x.filter(|v| v.is_finite() && *v > 0.0);
    let (a, b, r) = (ok(ng_sparse), ok(ng_klu), ok(rs));
    let Some(r) = r else {
        return Ratio::default();
    };
    let (fast, name) = match (a, b) {
        (Some(a), Some(b)) => {
            if a <= b {
                (Some(a), Some("sparse"))
            } else {
                (Some(b), Some("klu"))
            }
        }
        (Some(a), None) => (Some(a), Some("sparse")),
        (None, Some(b)) => (Some(b), Some("klu")),
        (None, None) => (None, None),
    };
    Ratio {
        vs_sparse: a.map(|a| a / r),
        vs_klu: b.map(|b| b / r),
        vs_fastest: fast.map(|f| f / r),
        fastest: name.map(str::to_string),
    }
}

/// Net-of-floor ratio is reported only when the RustSpice net time exceeds this many standard deviations of the
/// RustSpice start-up floor (the floor is measured once, under a different load than the case it is subtracted from).
pub const NET_SPREAD_FACTOR: f64 = 5.0;

/// Speedup after subtracting each tool's measured process start-up floor from its end-to-end time. `None` (n/a) when a
/// net time is not positive, or when the RustSpice net time is below 20 microseconds or below
/// [`NET_SPREAD_FACTOR`] x the spread (standard deviation) of the RustSpice floor: a ratio built on a few hundred
/// microseconds of drift between the floor measurement and the case is noise, not a speedup.
pub fn net_ratio(
    ng_med: f64,
    ng_floor: f64,
    rs_med: f64,
    rs_floor: f64,
    rs_floor_spread: f64,
) -> Option<f64> {
    let (n, r) = (ng_med - ng_floor, rs_med - rs_floor);
    let guard = 20.0f64.max(NET_SPREAD_FACTOR * rs_floor_spread.max(0.0));
    (n > 0.0 && r >= guard && n.is_finite() && r.is_finite() && rs_floor_spread.is_finite())
        .then_some(n / r)
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SpeedupSet {
    pub e2e_median: Ratio,
    pub e2e_min: Ratio,
    /// ngspice `Total analysis time` / RustSpice `analysis_us`. NOT like-for-like: the RustSpice figure excludes its
    /// `compile_us` (symbolic analysis / JIT), the ngspice figure includes ngspice's matrix setup.
    pub analysis_median: Ratio,
    pub analysis_min: Ratio,
    /// ngspice `Total analysis time` / RustSpice `compile_us + analysis_us`: the like-for-like analysis ratio
    pub like_for_like_median: Ratio,
    pub like_for_like_min: Ratio,
    /// e2e speedup vs the faster ngspice solver after subtracting each tool's start-up floor (median based);
    /// null when the RustSpice net time is within the floor's spread (see [`net_ratio`])
    pub e2e_net_of_startup: Option<f64>,
}

fn find<'a>(cs: &'a [ContenderStats], name: &str) -> Option<&'a ContenderStats> {
    cs.iter().find(|c| c.name == name)
}

/// Contender names of the b1..b4 timing blocks.
pub const NG_SPARSE: &str = "ngspice-sparse";
pub const NG_KLU: &str = "ngspice-klu";
/// Prefix of the b5 `.sens` contenders (they compute and write more than the requested gradients).
pub const NG_SENS_PREFIX: &str = "ngspice-sens-";
pub const RS: &str = "rustspice";

#[derive(Clone, Debug, Default, Serialize)]
pub struct Floors {
    pub status: String,
    pub reps: usize,
    pub ngspice_sparse_us: Option<Summary>,
    pub ngspice_klu_us: Option<Summary>,
    pub rustspice_us: Option<Summary>,
}

impl Floors {
    fn med(s: &Option<Summary>) -> Option<f64> {
        s.as_ref().map(|s| s.median)
    }
    fn spread(s: &Option<Summary>) -> Option<f64> {
        s.as_ref().map(|s| s.stddev)
    }
}

#[derive(Clone, Copy)]
enum Metric {
    E2e,
    /// the tool's own analysis time (RustSpice: without compile)
    Analysis,
    /// RustSpice `compile + analysis` against ngspice `Total analysis time`
    LikeForLike,
}

#[derive(Clone, Copy)]
enum Stat {
    Median,
    Min,
}

/// A statistic of a contender that completed every scheduled repetition: a partial sample (a repetition failed) or an
/// unsuccessful run never produces a ratio.
fn value_of(cs: &[ContenderStats], name: &str, m: Metric, st: Stat) -> Option<f64> {
    let c = find(cs, name).filter(|c| c.completed())?;
    let s = match m {
        Metric::E2e => c.e2e_us.as_ref(),
        Metric::Analysis => c.analysis_us.as_ref(),
        Metric::LikeForLike if name == RS => c.compile_analysis_us.as_ref(),
        Metric::LikeForLike => c.analysis_us.as_ref(),
    }?;
    Some(match st {
        Stat::Median => s.median,
        Stat::Min => s.min,
    })
}

fn ratio_of(cs: &[ContenderStats], m: Metric, st: Stat) -> Ratio {
    ratio(
        value_of(cs, NG_SPARSE, m, st),
        value_of(cs, NG_KLU, m, st),
        value_of(cs, RS, m, st),
    )
}

pub fn speedups(cs: &[ContenderStats], floors: &Floors) -> SpeedupSet {
    let med_e2e = |n: &str| value_of(cs, n, Metric::E2e, Stat::Median);
    // net time of the faster ngspice solver after removing its own start-up floor
    let best_ng = [
        (med_e2e(NG_SPARSE), Floors::med(&floors.ngspice_sparse_us)),
        (med_e2e(NG_KLU), Floors::med(&floors.ngspice_klu_us)),
    ]
    .into_iter()
    .filter_map(|(t, f)| Some((t?, f?)))
    .min_by(|a, b| {
        (a.0 - a.1)
            .partial_cmp(&(b.0 - b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let net = match (
        med_e2e(RS),
        Floors::med(&floors.rustspice_us),
        Floors::spread(&floors.rustspice_us),
        best_ng,
    ) {
        (Some(rs), Some(rf), Some(sd), Some((t, f))) => net_ratio(t, f, rs, rf, sd),
        _ => None,
    };
    SpeedupSet {
        e2e_median: ratio_of(cs, Metric::E2e, Stat::Median),
        e2e_min: ratio_of(cs, Metric::E2e, Stat::Min),
        analysis_median: ratio_of(cs, Metric::Analysis, Stat::Median),
        analysis_min: ratio_of(cs, Metric::Analysis, Stat::Min),
        like_for_like_median: ratio_of(cs, Metric::LikeForLike, Stat::Median),
        like_for_like_min: ratio_of(cs, Metric::LikeForLike, Stat::Min),
        e2e_net_of_startup: net,
    }
}

/// Names of the contenders whose repetitions were flagged noisy (stddev / median > 0.2) in e2e time.
pub fn noisy_names(cs: &[ContenderStats]) -> Vec<String> {
    cs.iter()
        .filter(|c| c.e2e_us.as_ref().is_some_and(|s| s.noisy))
        .map(|c| c.name.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Process runners
// ---------------------------------------------------------------------------------------------------------

pub struct Ctx {
    pub exe: PathBuf,
    pub ng: String,
    pub ng_ok: bool,
    pub ng_version: Option<String>,
    pub tmp: TmpDir,
    pub reps: usize,
    pub timing: bool,
    pub lim: Limits,
    pub lim_free: Limits,
    pub threads: usize,
    pub floors: Floors,
}

fn spawn_msg(e: rustsim::SpawnError) -> String {
    match e {
        rustsim::SpawnError::NotFound(m) | rustsim::SpawnError::Other(m) => format!("spawn: {m}"),
    }
}

fn ng_msg(c: &NgClass) -> String {
    match c {
        NgClass::Ok => "ok".to_string(),
        NgClass::Error(m) => m.clone(),
        NgClass::Timeout => "timeout".to_string(),
        NgClass::NotInstalled => "ngspice is not installed".to_string(),
    }
}

fn exit_msg(c: &ChildRun) -> String {
    let tail = c
        .stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("(no message)");
    match &c.exit {
        Exit::Code(0) => "ok".to_string(),
        Exit::Code(n) => format!("exit {n}: {tail}"),
        Exit::Signal(s) => format!("signal {s}: {tail}"),
        Exit::Timeout => "timeout".to_string(),
        Exit::RssCap => "killed at the RSS cap".to_string(),
    }
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Resident size at exit (KiB) from ngspice's own `Current ngspice program size = <x> MB` line (the `ru_maxrss` of a
/// spawned child is contaminated by the parent's high-water mark, so it is not used).
pub fn parse_ng_size_kib(stdout: &str) -> Option<u64> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with("Current ngspice program size ="))?;
    let v: f64 = line
        .split('=')
        .nth(1)?
        .trim()
        .trim_end_matches('.')
        .trim_end_matches("MB")
        .trim()
        .parse()
        .ok()?;
    Some((v * 1024.0).round() as u64)
}

/// `ngspice -b -r raw deck` as a contender (the raw file is written on tmpfs, as for RustSpice).
fn ng_run_sample(
    ctx: &Ctx,
    deck: &Path,
    raw: &Path,
    solver: NgSolver,
    lim: &Limits,
    want: bool,
) -> Result<Sample, String> {
    let r = ngspice::run(&ctx.ng, deck, raw, lim, Some(solver), want);
    if r.class != NgClass::Ok {
        return Err(ng_msg(&r.class));
    }
    Ok(Sample {
        e2e_us: r.child.wall_us(),
        analysis_us: r.log.analysis_s.filter(|s| *s > 0.0).map(|s| s * 1e6),
        rss_kib: parse_ng_size_kib(&r.child.stdout),
        info: r.log.solver.map(|s| s.as_str().to_string()),
        output: if want {
            r.plots.ok().map(Output::Plots)
        } else {
            None
        },
        ..Default::default()
    })
}

/// `rustspice run deck --raw --stats` as a contender.
fn rs_run_sample(
    ctx: &Ctx,
    deck: &Path,
    dir: &Path,
    tag: &str,
    lim: &Limits,
    want: bool,
) -> Result<Sample, String> {
    let r = rustsim::run(&ctx.exe, deck, dir, tag, lim).map_err(spawn_msg)?;
    match &r.class {
        RsClass::Ok => {}
        RsClass::LoudError(m) | RsClass::Panic(m) => return Err(m.clone()),
        RsClass::Timeout => return Err("timeout".to_string()),
    }
    let st = r.stats.as_ref();
    Ok(Sample {
        e2e_us: r.child.wall_us(),
        analysis_us: st.and_then(|s| s.analysis_us),
        parse_us: st.and_then(|s| s.parse_us),
        compile_us: st.and_then(|s| s.compile_us),
        write_us: st.and_then(|s| s.write_us),
        rss_kib: st.and_then(|s| s.peak_rss_kib),
        threads: None,
        info: st.and_then(|s| s.solver_kind.clone()),
        output: if want {
            rustsim::load_plots(&r).ok().map(Output::Plots)
        } else {
            None
        },
    })
}

fn ng_contender<'a>(ctx: &'a Ctx, solver: NgSolver, deck: PathBuf, raw: PathBuf) -> Contender<'a> {
    let name = match solver {
        NgSolver::Sparse => NG_SPARSE,
        NgSolver::Klu => NG_KLU,
    };
    Contender {
        name: name.to_string(),
        note: format!(
            "ngspice -b -r, {} solver, default tolerances",
            solver.as_str()
        ),
        launches: 1,
        threads: 1,
        heavy: false,
        run: Box::new(move |want| ng_run_sample(ctx, &deck, &raw, solver, &ctx.lim, want)),
    }
}

fn rs_contender<'a>(ctx: &'a Ctx, deck: PathBuf, dir: PathBuf) -> Contender<'a> {
    Contender {
        name: RS.to_string(),
        note: "rustspice run --raw --stats".to_string(),
        launches: 1,
        threads: 1,
        heavy: false,
        run: Box::new(move |want| rs_run_sample(ctx, &deck, &dir, "rs", &ctx.lim, want)),
    }
}

/// Run `n_jobs` jobs on at most `workers` threads; returns the wall time of the whole section and the first error.
fn run_parallel<F>(n_jobs: usize, workers: usize, job: F) -> (Duration, Option<String>, Option<u64>)
where
    F: Fn(usize) -> Result<Option<u64>, String> + Sync,
{
    let next = AtomicUsize::new(0);
    let first_err: Mutex<Option<String>> = Mutex::new(None);
    let max_rss: Mutex<Option<u64>> = Mutex::new(None);
    let t0 = Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..workers.max(1).min(n_jobs.max(1)) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n_jobs {
                    break;
                }
                match job(i) {
                    Ok(rss) => {
                        let mut m = max_rss.lock().unwrap();
                        *m = (*m).max(rss);
                    }
                    Err(e) => {
                        first_err.lock().unwrap().get_or_insert(e);
                    }
                }
            });
        }
    });
    let wall = t0.elapsed();
    (
        wall,
        first_err.into_inner().unwrap(),
        max_rss.into_inner().unwrap(),
    )
}

/// Start-up floors: trivial `.op` deck, both ngspice solvers and RustSpice, interleaved.
fn measure_floors(ctx: &Ctx) -> Floors {
    let dir = ctx.tmp.path().join("floor");
    if let Err(e) = fs::create_dir_all(&dir) {
        return Floors {
            status: format!("n/a: {e}"),
            ..Default::default()
        };
    }
    let deck = dir.join("floor.sp");
    let deck_klu = dir.join("floor_klu.sp");
    let text = floor_deck();
    if write_text(&deck, &text).is_err()
        || write_text(&deck_klu, &meta::inject_options(&text, "klu")).is_err()
    {
        return Floors {
            status: "n/a: cannot write floor decks".to_string(),
            ..Default::default()
        };
    }
    let mut cs = vec![rs_contender(ctx, deck.clone(), dir.clone())];
    if ctx.ng_ok {
        cs.push(ng_contender(
            ctx,
            NgSolver::Sparse,
            deck.clone(),
            dir.join("s.raw"),
        ));
        cs.push(ng_contender(
            ctx,
            NgSolver::Klu,
            deck_klu,
            dir.join("k.raw"),
        ));
    }
    let reps = ctx.reps.max(15);
    let ms = measure(&mut cs, reps, true);
    let st: Vec<ContenderStats> = ms.iter().map(stats_of).collect();
    let get = |n: &str| find(&st, n).and_then(|c| c.e2e_us.clone());
    Floors {
        status: if st.iter().all(|c| c.status == "ok") {
            "ok".to_string()
        } else {
            st.iter()
                .find(|c| c.status != "ok")
                .map_or(String::new(), |c| format!("{}: {}", c.name, c.status))
        },
        reps,
        ngspice_sparse_us: get(NG_SPARSE),
        ngspice_klu_us: get(NG_KLU),
        rustspice_us: get(RS),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Accuracy
// ---------------------------------------------------------------------------------------------------------

/// Accuracy of one case against the tight ngspice reference (and the independent oracle where one exists).
#[derive(Clone, Debug, Default, Serialize)]
pub struct Accuracy {
    /// `ok` or `n/a: <reason>`
    pub status: String,
    /// `max |dV| (V)` for op / dc, `max |dV| / swing` for tran
    pub metric: String,
    pub probes_total: usize,
    pub probes_compared: usize,
    pub probes_vacuous: usize,
    /// Headline error of RustSpice vs the reference (probes with a non-vacuous reference only)
    pub rs_err: Option<f64>,
    /// Same for ngspice at default tolerances (as-is SPARSE run)
    pub ng_err: Option<f64>,
    /// max |RustSpice - reference| in volts over ALL probes
    pub rs_abs_v: Option<f64>,
    pub ng_abs_v: Option<f64>,
    pub rs_le_ng: Option<bool>,
    /// What `rs_le_ng` compares: `independent oracle` where one exists, else `tight reference` (whose own error then
    /// bounds the RustSpice figure from below)
    pub rs_le_ng_basis: Option<String>,
    /// Transients: largest 50 %-crossing time shift against the reference (seconds), each waveform's crossings
    /// interpolated between its own rawfile rows
    pub rs_edge_shift_s: Option<f64>,
    pub ng_edge_shift_s: Option<f64>,
    /// The same with the reference read at the tool's own rows: the simulator's timing error alone
    pub rs_edge_shift_at_rows_s: Option<f64>,
    pub ng_edge_shift_at_rows_s: Option<f64>,
    /// Shift of the reference's crossings when it is read only at the tool's rows: the resolution of those rows
    pub rs_edge_row_floor_s: Option<f64>,
    pub ng_edge_row_floor_s: Option<f64>,
    /// Transients without an oracle: the reference judged against a still tighter ngspice run (same metric as `rs_err`)
    pub ref_selfcheck: Option<f64>,
    pub ref_selfcheck_label: Option<String>,
    pub worst_probe: Option<String>,
    pub worst_at: Option<f64>,
    pub oracle: Option<String>,
    pub oracle_status: Option<String>,
    pub oracle_metric: Option<String>,
    pub rs_vs_oracle: Option<f64>,
    pub ng_vs_oracle: Option<f64>,
    pub ref_vs_oracle: Option<f64>,
    /// `tight` or `fell_back: <reason>`
    pub ref_status: String,
    pub ref_label: String,
    pub ref_wall_us: Option<f64>,
    pub interp_floor: Option<f64>,
    pub points_rs: Option<usize>,
    pub points_ref: Option<usize>,
    /// Rows of the as-is ngspice transient plot: `ngspice -b -r` writes its own adaptive time points
    pub points_ng: Option<usize>,
    pub notes: Vec<String>,
}

impl Accuracy {
    fn na(reason: impl Into<String>) -> Accuracy {
        Accuracy {
            status: format!("n/a: {}", reason.into()),
            ref_status: "n/a".to_string(),
            ..Default::default()
        }
    }
}

/// The ngspice reference run of a case.
pub struct RefRun {
    pub plots: Result<Vec<Plot>, String>,
    pub label: String,
    pub wall_us: Option<f64>,
}

fn reference_deck(
    deck: &str,
    kind: PlotKind,
    tmax_override: Option<f64>,
    options: Option<&str>,
) -> Result<(String, String), String> {
    match kind {
        PlotKind::Tran => {
            let card = meta::find_tran(deck).ok_or("deck has no parsable .tran card")?;
            let tmax = tmax_override.unwrap_or(card.tstep / 20.0);
            let d = meta::set_tran_tmax(deck, tmax)?;
            let opts = options.unwrap_or(TRAN_REF_OPTIONS);
            Ok((
                meta::inject_options(&d, opts),
                format!("{opts}, tmax = {tmax:e} s"),
            ))
        }
        _ => {
            let opts = options.unwrap_or(TIGHT_OPTIONS);
            Ok((meta::inject_options(deck, opts), opts.to_string()))
        }
    }
}

/// Run a tight reference (ngspice SPARSE) for a deck; `tag` names the scratch files.
fn make_reference_tagged(
    ctx: &Ctx,
    deck: &str,
    kind: PlotKind,
    tmax: Option<f64>,
    options: Option<&str>,
    tag: &str,
    dir: &Path,
) -> RefRun {
    let (text, label) = match reference_deck(deck, kind, tmax, options) {
        Ok(x) => x,
        Err(e) => {
            return RefRun {
                plots: Err(e),
                label: String::new(),
                wall_us: None,
            }
        }
    };
    let path = dir.join(format!("{tag}.sp"));
    if let Err(e) = write_text(&path, &text) {
        return RefRun {
            plots: Err(e),
            label,
            wall_us: None,
        };
    }
    let lim = Limits {
        timeout: ctx.lim.timeout * 2,
        ..ctx.lim.clone()
    };
    let r = ngspice::run(
        &ctx.ng,
        &path,
        &dir.join(format!("{tag}.raw")),
        &lim,
        Some(NgSolver::Sparse),
        true,
    );
    let wall = Some(r.child.wall_us());
    let plots = match r.class {
        NgClass::Ok => r.plots,
        ref c => Err(ng_msg(c)),
    };
    RefRun {
        plots,
        label,
        wall_us: wall,
    }
}

fn make_reference(ctx: &Ctx, deck: &str, kind: PlotKind, tmax: Option<f64>, dir: &Path) -> RefRun {
    make_reference_tagged(ctx, deck, kind, tmax, None, "ref", dir)
}

fn plots_of(m: Option<&Measured>) -> Option<&[Plot]> {
    match m?.output.as_ref()? {
        Output::Plots(p) => Some(p.as_slice()),
        _ => None,
    }
}

fn compare_meta(
    id: &str,
    floor: f64,
    probes: Vec<String>,
    oracle: Option<&str>,
    edges: bool,
) -> Meta {
    let mut m = Meta::defaults(id);
    m.floor = floor;
    m.probes = probes;
    m.oracle = oracle.map(str::to_string);
    if edges {
        m.tags.push("timing_edge".to_string());
    }
    m
}

/// Headline numbers of one `compare` result: `(headline error, max abs error over all probes, worst probe and
/// location, largest edge shift, counts)`.
struct Folded {
    err: Option<f64>,
    abs_v: Option<f64>,
    edge: Option<f64>,
    edge_at_rows: Option<f64>,
    edge_row_floor: Option<f64>,
    worst: Option<(String, Option<f64>)>,
    total: usize,
    vacuous: usize,
    compared: usize,
}

fn fold_one(cr: &CompareResult) -> Folded {
    let worst = cr
        .probes
        .iter()
        .filter(|p| !p.vacuous && p.rs_err.is_some())
        .max_by(|a, b| {
            a.rs_err
                .partial_cmp(&b.rs_err)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| (p.name.clone(), p.worst_at));
    Folded {
        err: cr.err_rs,
        abs_v: cr.probes.iter().filter_map(|p| p.abs_err).reduce(f64::max),
        edge: cr
            .probes
            .iter()
            .filter(|p| !p.vacuous)
            .filter_map(|p| p.edge_shift)
            .reduce(f64::max),
        edge_at_rows: cr
            .probes
            .iter()
            .filter(|p| !p.vacuous)
            .filter_map(|p| p.edge_shift_at_rows)
            .reduce(f64::max),
        edge_row_floor: cr
            .probes
            .iter()
            .filter(|p| !p.vacuous)
            .filter_map(|p| p.edge_row_floor)
            .reduce(f64::max),
        worst,
        total: cr.probes.len(),
        vacuous: cr.probes.iter().filter(|p| p.vacuous).count(),
        compared: cr
            .probes
            .iter()
            .filter(|p| !p.vacuous && p.rs_err.is_some())
            .count(),
    }
}

/// Max |rs - truth| over the ladder nodes `v(1)..v(n+1)` of an `.op` plot.
pub fn ladder_plot_error(plot: &Plot, truth: &[f64]) -> Option<f64> {
    let mut worst = 0.0f64;
    for (k, t) in truth.iter().enumerate() {
        let v = *plot.col(&format!("v({})", k + 1))?.first()?;
        if !v.is_finite() {
            return None;
        }
        worst = worst.max((v - t).abs());
    }
    Some(worst)
}

// ---------------------------------------------------------------------------------------------------------
// b1..b4 cases
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CaseSpec {
    pub id: String,
    pub bench: &'static str,
    pub circuit: String,
    pub detail: String,
    pub size: usize,
    pub legacy: bool,
    pub kind: PlotKind,
    pub deck: String,
    pub floor: f64,
    /// `oracle.rs` spec and the probes it is evaluated on
    pub oracle: Option<(&'static str, Vec<String>)>,
    /// `(stages, shunt)` of a ladder: exact recursion oracle on every node
    pub ladder: Option<(usize, f64)>,
    pub ref_tmax: Option<f64>,
    /// Judge the reference against a still tighter ngspice run (transients without an oracle)
    pub ref_check: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub bench: String,
    pub circuit: String,
    pub detail: String,
    pub size: usize,
    pub legacy_size: bool,
    pub analysis: String,
    pub tstep_s: Option<f64>,
    pub deck_lines: usize,
    pub timing_status: String,
    pub reps: usize,
    pub contenders: Vec<ContenderStats>,
    pub speedups: Option<SpeedupSet>,
    pub noisy: Vec<String>,
    pub loadavg_start: Option<f64>,
    pub loadavg_end: Option<f64>,
    pub accuracy: Accuracy,
    pub rustspice_ok: bool,
    pub notes: Vec<String>,
}

impl CaseResult {
    fn new(spec: &CaseSpec) -> CaseResult {
        CaseResult {
            id: spec.id.clone(),
            bench: spec.bench.to_string(),
            circuit: spec.circuit.clone(),
            detail: spec.detail.clone(),
            size: spec.size,
            legacy_size: spec.legacy,
            analysis: spec.kind.as_str().to_string(),
            tstep_s: meta::find_tran(&spec.deck).map(|c| c.tstep),
            deck_lines: spec.deck.lines().count(),
            timing_status: String::new(),
            reps: 0,
            contenders: Vec::new(),
            speedups: None,
            noisy: Vec::new(),
            loadavg_start: None,
            loadavg_end: None,
            accuracy: Accuracy::default(),
            rustspice_ok: false,
            notes: Vec::new(),
        }
    }

    pub fn stat(&self, name: &str) -> Option<&ContenderStats> {
        find(&self.contenders, name)
    }
}

fn metric_name(kind: PlotKind) -> &'static str {
    match kind {
        PlotKind::Tran => "max |dV| / swing",
        _ => "max |dV| (V)",
    }
}

/// Run one b1..b4 case: timing (interleaved) and accuracy.
pub fn run_case(ctx: &Ctx, spec: &CaseSpec, shared_ref: Option<&RefRun>) -> CaseResult {
    let mut res = CaseResult::new(spec);
    res.loadavg_start = loadavg();
    let dir = ctx.tmp.path().join(&spec.id);
    let prep = fs::create_dir_all(&dir)
        .map_err(|e| e.to_string())
        .and_then(|_| write_text(&dir.join("sparse.sp"), &spec.deck))
        .and_then(|_| {
            write_text(
                &dir.join("klu.sp"),
                &meta::inject_options(&spec.deck, "klu"),
            )
        });
    if let Err(e) = prep {
        res.accuracy = Accuracy::na(e);
        res.timing_status = "n/a: setup failed".to_string();
        return res;
    }
    let mut cs: Vec<Contender> = Vec::new();
    if ctx.ng_ok {
        cs.push(ng_contender(
            ctx,
            NgSolver::Sparse,
            dir.join("sparse.sp"),
            dir.join("ng_s.raw"),
        ));
        cs.push(ng_contender(
            ctx,
            NgSolver::Klu,
            dir.join("klu.sp"),
            dir.join("ng_k.raw"),
        ));
    }
    cs.push(rs_contender(ctx, dir.join("sparse.sp"), dir.clone()));
    let measured = measure(&mut cs, ctx.reps, ctx.timing);
    drop(cs);
    res.contenders = measured.iter().map(stats_of).collect();
    res.rustspice_ok = measured.iter().any(|m| m.name == RS && m.ok());
    for m in measured.iter().filter(|m| !m.ok()) {
        res.notes.push(format!("{}: {}", m.name, m.status));
    }
    if ctx.timing {
        res.reps = res.contenders.iter().map(|c| c.n).max().unwrap_or(0);
        res.timing_status = if res.rustspice_ok && ctx.ng_ok {
            "ok".to_string()
        } else if !ctx.ng_ok {
            "n/a: ngspice is not installed (unverified)".to_string()
        } else {
            "n/a: rustspice failed".to_string()
        };
        res.speedups = Some(speedups(&res.contenders, &ctx.floors));
        res.noisy = noisy_names(&res.contenders);
    } else {
        res.timing_status = "skipped (--no-timing)".to_string();
    }

    // ---- accuracy
    let own_ref;
    let rf: Option<&RefRun> = if !ctx.ng_ok {
        None
    } else if let Some(r) = shared_ref {
        Some(r)
    } else {
        own_ref = make_reference(ctx, &spec.deck, spec.kind, spec.ref_tmax, &dir);
        Some(&own_ref)
    };
    let rs_plots = plots_of(measured.iter().find(|m| m.name == RS));
    let ng_plots = plots_of(measured.iter().find(|m| m.name == NG_SPARSE));
    res.accuracy = match (rf, rs_plots) {
        (None, _) => Accuracy::na("ngspice is not installed (unverified)"),
        (_, None) => Accuracy::na("rustspice produced no rawfile"),
        (Some(rf), Some(rs)) => accuracy_of(ctx, &dir, spec, rs, ng_plots, rf),
    };
    res.loadavg_end = loadavg();
    res
}

fn accuracy_of(
    ctx: &Ctx,
    dir: &Path,
    spec: &CaseSpec,
    rs: &[Plot],
    ng: Option<&[Plot]>,
    rf: &RefRun,
) -> Accuracy {
    let mut acc = Accuracy {
        metric: metric_name(spec.kind).to_string(),
        ref_label: rf.label.clone(),
        ref_wall_us: rf.wall_us,
        ..Default::default()
    };
    // Reference: tight run, else fall back to the as-is run (then ngspice's own error is not measurable).
    let mut fell_back = false;
    let ref_plots: &[Plot] = match (&rf.plots, ng) {
        (Ok(p), _) => {
            acc.ref_status = "tight".to_string();
            p
        }
        (Err(e), Some(n)) => {
            fell_back = true;
            acc.ref_status = format!("fell_back: {e}");
            acc.notes
                .push("reference run failed: compared against the as-is ngspice run".to_string());
            n
        }
        (Err(e), None) => return Accuracy::na(format!("no ngspice reference ({e})")),
    };
    let edges = spec.kind == PlotKind::Tran;
    let m = compare_meta(&spec.id, spec.floor, Vec::new(), None, edges);
    let cmp = |a: &[Plot], b: &[Plot]| {
        compare(&CompareInput {
            meta: &m,
            deck: &spec.deck,
            rs: a,
            reference: b,
            asis: None,
        })
    };
    // RustSpice and the as-is ngspice run are judged by the same code against the same reference.
    let cr_rs = cmp(rs, ref_plots);
    let f = fold_one(&cr_rs);
    acc.probes_total = f.total;
    acc.probes_vacuous = f.vacuous;
    acc.probes_compared = f.compared;
    acc.rs_err = f.err;
    acc.rs_abs_v = f.abs_v;
    acc.rs_edge_shift_s = f.edge;
    acc.rs_edge_shift_at_rows_s = f.edge_at_rows;
    acc.rs_edge_row_floor_s = f.edge_row_floor;
    if let Some((name, at)) = f.worst {
        acc.worst_probe = Some(name);
        acc.worst_at = at;
    }
    acc.interp_floor = cr_rs.interp_floor;
    acc.points_rs = cr_rs.points_rs;
    acc.points_ref = cr_rs.points_ref;
    if edges {
        acc.points_ng = ng
            .and_then(|n| crate::bench::rawfile::last_plot(n, PlotKind::Tran))
            .map(|p| p.npoints);
    }
    for f in cr_rs
        .failures
        .iter()
        .filter(|f| f.contains("missing"))
        .take(2)
    {
        acc.notes.push(f.clone());
    }
    if let (false, Some(n)) = (fell_back, ng) {
        let g = fold_one(&cmp(n, ref_plots));
        acc.ng_err = g.err;
        acc.ng_abs_v = g.abs_v;
        acc.ng_edge_shift_s = g.edge;
        acc.ng_edge_shift_at_rows_s = g.edge_at_rows;
        acc.ng_edge_row_floor_s = g.edge_row_floor;
    }
    if let (Some(a), Some(b)) = (acc.rs_err, acc.ng_err) {
        acc.rs_le_ng = Some(a <= b);
        acc.rs_le_ng_basis = Some("tight reference".to_string());
    }
    acc.status = if acc.rs_err.is_some() || acc.rs_abs_v.is_some() {
        "ok".to_string()
    } else {
        format!(
            "n/a: no comparable probe ({})",
            cr_rs
                .failures
                .first()
                .or(cr_rs.notes.first())
                .map_or("no detail", String::as_str)
        )
    };
    // ---- independent oracles
    if let Some((stages, rsh)) = spec.ladder {
        acc.oracle = Some("exact ladder recursion".to_string());
        acc.oracle_metric = Some("max |dV| (V), all nodes".to_string());
        let truth = ladder_voltages(stages, R_SER, rsh, 5.0);
        let last = |p: &[Plot]| {
            crate::bench::rawfile::last_plot(p, PlotKind::Op)
                .and_then(|pl| ladder_plot_error(pl, &truth))
        };
        acc.rs_vs_oracle = last(rs);
        acc.ng_vs_oracle = ng.and_then(last);
        acc.ref_vs_oracle = if fell_back { None } else { last(ref_plots) };
        acc.oracle_status = Some(if acc.rs_vs_oracle.is_some() {
            "ok".to_string()
        } else {
            "n/a: node vectors missing".to_string()
        });
    } else if let Some((spec_str, probes)) = &spec.oracle {
        let mo = compare_meta(&spec.id, spec.floor, probes.clone(), Some(spec_str), false);
        let cr_o = compare(&CompareInput {
            meta: &mo,
            deck: &spec.deck,
            rs,
            reference: ref_plots,
            asis: ng,
        });
        acc.oracle = Some((*spec_str).to_string());
        acc.oracle_metric = cr_o.oracle.as_ref().map(|o| o.metric.clone());
        acc.oracle_status = cr_o.oracle.as_ref().map(|o| match &o.reason {
            Some(r) => format!("{}: {r}", o.status),
            None => o.status.clone(),
        });
        acc.rs_vs_oracle = cr_o.oracle.as_ref().and_then(|o| o.rs_max_err);
        acc.ng_vs_oracle = cr_o.oracle.as_ref().and_then(|o| o.ng_max_err);
        if !fell_back {
            // the reference itself, judged by the oracle (the reference plots stand in for the contender)
            let cr_r = compare(&CompareInput {
                meta: &mo,
                deck: &spec.deck,
                rs: ref_plots,
                reference: ref_plots,
                asis: None,
            });
            acc.ref_vs_oracle = cr_r.oracle.as_ref().and_then(|o| o.rs_max_err);
        }
    }
    // an independent oracle is a better judge than the reference, whose own error bounds the RustSpice figure below
    if acc.oracle_status.as_deref() == Some("ok") {
        if let (Some(a), Some(b)) = (acc.rs_vs_oracle, acc.ng_vs_oracle) {
            acc.rs_le_ng = Some(a <= b);
            acc.rs_le_ng_basis = Some("independent oracle".to_string());
        }
    }
    // ---- reference self-convergence: the reference against a still tighter ngspice run
    if spec.ref_check && !fell_back {
        let tstep = meta::find_tran(&spec.deck).map_or(0.0, |c| c.tstep);
        let tighter = make_reference_tagged(
            ctx,
            &spec.deck,
            spec.kind,
            Some(tstep / 80.0),
            Some("reltol=1e-7 abstol=1e-16 vntol=1e-10"),
            "ref2",
            dir,
        );
        acc.ref_selfcheck_label = Some(tighter.label.clone());
        match &tighter.plots {
            Ok(p2) => acc.ref_selfcheck = fold_one(&cmp(ref_plots, p2)).err,
            Err(e) => acc
                .notes
                .push(format!("reference self-check run failed: {e}")),
        }
    }
    acc
}

/// The b1..b4 case list (legacy sizes last in each ladder).
pub fn case_specs(only: &[String]) -> Vec<CaseSpec> {
    let want = |b: &str| only.is_empty() || only.iter().any(|o| o.eq_ignore_ascii_case(b));
    let mut v = Vec::new();
    if want("b1") {
        for &n in &[500usize, 2000, 5000] {
            let rsh = R_SH_LEGACY;
            v.push(CaseSpec {
                id: format!("b1_ladder_{n}"),
                bench: "b1",
                circuit: format!(
                    "{}-node power-grid ladder, 10 kohm shunts (legacy circuit)",
                    fmt_count(n as u64)
                ),
                detail: format!(
                    "{} resistors, {} MNA unknowns",
                    fmt_count(2 * n as u64),
                    fmt_count(n as u64 + 2)
                ),
                size: n,
                legacy: n == 5000,
                kind: PlotKind::Op,
                deck: ladder_deck(n, rsh),
                floor: 1e-6,
                oracle: None,
                ladder: Some((n, rsh)),
                ref_tmax: None,
                ref_check: false,
            });
        }
        for &n in &[500usize, 2000, 5000] {
            let rsh = shunt_for_depth(n, 2.0);
            v.push(CaseSpec {
                id: format!("b1n_ladder_flat_{n}"),
                bench: "b1",
                circuit: format!(
                    "{}-node ladder, shunt scaled so every node is O(1) V",
                    fmt_count(n as u64)
                ),
                detail: format!(
                    "{} resistors, {} MNA unknowns, shunt {} ohm",
                    fmt_count(2 * n as u64),
                    fmt_count(n as u64 + 2),
                    fmt_count(rsh.round() as u64)
                ),
                size: n,
                legacy: false,
                kind: PlotKind::Op,
                deck: ladder_deck(n, rsh),
                floor: 1e-6,
                oracle: None,
                ladder: Some((n, rsh)),
                ref_tmax: None,
                ref_check: false,
            });
        }
    }
    if want("b2") {
        for &n in &[100usize, 500, 2000] {
            let probes: Vec<String> = [2, n / 4 + 1, n / 2 + 1, n + 1]
                .iter()
                .map(|k| format!("v({k})"))
                .collect();
            v.push(CaseSpec {
                id: format!("b2_rcline_{n}"),
                bench: "b2",
                circuit: format!("{}-stage RC line, .tran 10p 5n", fmt_count(n as u64)),
                detail: format!(
                    "{} R/C elements, `.tran 10p 5n` (500 nominal steps)",
                    fmt_count(2 * n as u64)
                ),
                size: n,
                legacy: n == 2000,
                kind: PlotKind::Tran,
                deck: rc_line_deck(n, "10p"),
                floor: TRAN_FLOOR_V,
                oracle: (n <= TRAN_ORACLE_MAX_STAGES).then_some(("tran_lti", probes)),
                ladder: None,
                ref_tmax: None,
                ref_check: false,
            });
        }
    }
    if want("b3") {
        for &n in &[16usize, 32, 64] {
            let probes: Vec<String> = [n + 2, n + 1 + n / 2, 2 * n + 1]
                .iter()
                .map(|k| format!("v({k})"))
                .collect();
            v.push(CaseSpec {
                id: format!("b3_diodes_{n}"),
                bench: "b3",
                circuit: format!("{n}-diode cascade, 201-point .dc sweep"),
                detail: format!("{n} diodes + {} resistors, 201 sweep points", 2 * n),
                size: n,
                legacy: n == 64,
                kind: PlotKind::Dc,
                deck: diode_cascade_deck(n),
                floor: 1e-6,
                oracle: Some(("dc_nonlinear", probes)),
                ladder: None,
                ref_tmax: None,
                ref_check: false,
            });
        }
    }
    if want("b4") {
        for &n in &[25usize, 50, 100] {
            v.push(CaseSpec {
                id: format!("b4_cmos_{n}"),
                bench: "b4",
                circuit: format!("{n}-stage CMOS inverter chain, .tran 5p 2n"),
                detail: format!(
                    "{} MOSFETs + {n} caps, `.tran 5p 2n` (400 nominal steps)",
                    2 * n
                ),
                size: n,
                legacy: n == 100,
                kind: PlotKind::Tran,
                deck: cmos_chain_deck(n),
                floor: TRAN_FLOOR_V,
                oracle: None,
                ladder: None,
                ref_tmax: None,
                ref_check: true,
            });
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------------------
// In-process workers (the same binary re-executed: `bench-ngspice --worker <job.json>`)
// ---------------------------------------------------------------------------------------------------------

/// One RustSpice workload executed in a child process so that its end-to-end time includes process start-up, parse,
/// compile and result output, exactly like `rustspice run --raw` and `ngspice -b -r`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkerJob {
    /// `adjoint` | `fd` | `batch` | `seq`
    pub kind: String,
    pub deck: PathBuf,
    pub out: PathBuf,
    pub target: Option<String>,
    pub h_rel: Option<f64>,
    /// Corner values of `V1` (batch / seq)
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkerOut {
    pub parse_us: f64,
    pub compile_us: f64,
    /// Time of the analysis calls only (no parse, compile or output)
    pub solve_us: f64,
    /// Threads the job's analysis calls ran on (the rayon pool size for `batch`, 1 for the sequential jobs)
    pub threads: usize,
    /// `VmHWM` of the worker process itself
    pub peak_rss_kib: Option<u64>,
    /// Resistor names (gradients) or `v(node)` labels (batch / seq)
    pub names: Vec<String>,
    pub values: Vec<f64>,
    pub rows: Vec<Vec<f64>>,
}

fn worker_threads() -> usize {
    std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

fn us_since(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e6
}

pub fn run_worker_job(job: &WorkerJob) -> Result<WorkerOut, String> {
    use rustspice_backend_cpu::{BatchCircuitInput, CpuSimdBackend};
    use rustspice_solver::{ParameterGradSpec, Simulator};

    let src = fs::read_to_string(&job.deck)
        .map_err(|e| format!("cannot read {}: {e}", job.deck.display()))?;
    let t0 = Instant::now();
    let netlist = rustspice_core::parse_netlist(&src).map_err(|e| e.to_string())?;
    let parse_us = us_since(t0);
    let t1 = Instant::now();
    let mut sim = Simulator::from_netlist(netlist, Arc::new(CpuSimdBackend::new()))
        .map_err(|e| e.to_string())?;
    let compile_us = us_since(t1);
    let mut out = WorkerOut {
        parse_us,
        compile_us,
        // only the batch API runs on the rayon pool; every other worker job is one sequential loop on one thread
        threads: if job.kind == "batch" {
            worker_threads()
        } else {
            1
        },
        ..Default::default()
    };
    let target = job.target.clone();
    let need_target = || {
        target
            .clone()
            .ok_or_else(|| "job needs a target node".to_string())
    };
    let n_volt = sim.netlist.node_names.len().saturating_sub(1);
    let labels: Vec<String> = sim
        .netlist
        .node_names
        .iter()
        .skip(1)
        .map(|n| format!("v({})", n.to_ascii_lowercase()))
        .collect();
    match job.kind.as_str() {
        "adjoint" => {
            let names = sim.topology.soa.res_names.clone();
            let specs: Vec<ParameterGradSpec> = names
                .iter()
                .map(|n| ParameterGradSpec::new(n.clone(), "r"))
                .collect();
            let t = Instant::now();
            let (_op, adj) = sim
                .run_dc_adjoint(&need_target()?, &specs)
                .map_err(|e| e.to_string())?;
            out.solve_us = us_since(t);
            out.names = names;
            out.values = adj.gradients;
        }
        "fd" => {
            let target = need_target()?;
            let h = job.h_rel.unwrap_or(FD_H_CENTRAL);
            let names = sim.topology.soa.res_names.clone();
            let nominal: Vec<f64> = (0..names.len())
                .map(|i| sim.topology.soa.res_resistance[i])
                .collect();
            let volt = |sim: &mut Simulator| -> Result<f64, String> {
                let op = sim.run_dc_op().map_err(|e| e.to_string())?;
                sim.node_voltage(&op.solution, &target)
                    .ok_or_else(|| format!("node `{target}` not found"))
            };
            let t = Instant::now();
            for (i, name) in names.iter().enumerate() {
                let (rp, rm) = (nominal[i] * (1.0 + h), nominal[i] * (1.0 - h));
                sim.topology
                    .soa
                    .set_device_parameter(name, "r", rp)
                    .map_err(|e| e.to_string())?;
                let vp = volt(&mut sim)?;
                sim.topology
                    .soa
                    .set_device_parameter(name, "r", rm)
                    .map_err(|e| e.to_string())?;
                let vm = volt(&mut sim)?;
                sim.topology
                    .soa
                    .set_device_parameter(name, "r", nominal[i])
                    .map_err(|e| e.to_string())?;
                out.values.push((vp - vm) / (rp - rm));
            }
            out.solve_us = us_since(t);
            out.names = names;
        }
        "batch" => {
            let inputs: Vec<BatchCircuitInput> = job
                .values
                .iter()
                .map(|&v| BatchCircuitInput {
                    overrides: vec![("V1".to_string(), "dc".to_string(), v)],
                })
                .collect();
            let t = Instant::now();
            let outs = sim.run_batch_dc_op(&inputs).map_err(|e| e.to_string())?;
            out.solve_us = us_since(t);
            out.rows = outs
                .into_iter()
                .map(|o| o.solution[..n_volt].to_vec())
                .collect();
            out.names = labels;
        }
        "seq" => {
            let t = Instant::now();
            for &v in &job.values {
                sim.topology
                    .soa
                    .set_device_parameter("V1", "dc", v)
                    .map_err(|e| e.to_string())?;
                let op = sim.run_dc_op().map_err(|e| e.to_string())?;
                out.rows.push(op.solution[..n_volt].to_vec());
            }
            out.solve_us = us_since(t);
            out.names = labels;
        }
        other => return Err(format!("unknown worker kind `{other}`")),
    }
    out.peak_rss_kib = peak_rss_kib();
    Ok(out)
}

/// Entry point of `--worker`: run the job, write its JSON result, return the exit code.
pub fn worker_main(job_path: &Path) -> i32 {
    let go = || -> Result<(), String> {
        let text = fs::read_to_string(job_path)
            .map_err(|e| format!("cannot read job {}: {e}", job_path.display()))?;
        let job: WorkerJob = serde_json::from_str(&text).map_err(|e| format!("bad job: {e}"))?;
        let out = run_worker_job(&job)?;
        let json = serde_json::to_string(&out).map_err(|e| e.to_string())?;
        fs::write(&job.out, json).map_err(|e| format!("cannot write {}: {e}", job.out.display()))
    };
    match go() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}

/// Run a worker job in a child process; returns the child record and the worker's result.
fn run_worker(
    ctx: &Ctx,
    dir: &Path,
    tag: &str,
    job: &WorkerJob,
    env: &[(&str, String)],
    lim: &Limits,
) -> Result<(ChildRun, WorkerOut), String> {
    let job_path = dir.join(format!("{tag}.job.json"));
    let json = serde_json::to_string(job).map_err(|e| e.to_string())?;
    write_text(&job_path, &json)?;
    let _ = fs::remove_file(&job.out);
    let mut cmd = Command::new(&ctx.exe);
    cmd.arg("bench-ngspice").arg("--worker").arg(&job_path);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let child = run_guarded(&mut cmd, lim).map_err(spawn_msg)?;
    if child.exit != Exit::Code(0) {
        return Err(exit_msg(&child));
    }
    let text = fs::read_to_string(&job.out).map_err(|e| format!("worker wrote no result: {e}"))?;
    let out: WorkerOut =
        serde_json::from_str(&text).map_err(|e| format!("bad worker result: {e}"))?;
    Ok((child, out))
}

fn worker_sample(child: &ChildRun, out: &WorkerOut, output: Option<Output>) -> Sample {
    Sample {
        e2e_us: child.wall_us(),
        analysis_us: Some(out.solve_us).filter(|v| *v > 0.0),
        parse_us: Some(out.parse_us),
        compile_us: Some(out.compile_us),
        write_us: None,
        rss_kib: out.peak_rss_kib,
        threads: Some(out.threads),
        info: Some(format!("{} thread(s)", out.threads)),
        output,
    }
}

/// Reorder worker gradients (named) into the resistor deck order.
fn order_by_names(want: &[String], names: &[String], values: &[f64]) -> Result<Vec<f64>, String> {
    want.iter()
        .map(|w| {
            names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(w))
                .and_then(|i| values.get(i).copied())
                .ok_or_else(|| format!("no gradient for `{w}`"))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// b5 / b6 result types
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkContender {
    pub stats: ContenderStats,
    /// b5: gradient accuracy against the dense f64 reference
    pub gradient: Option<GradError>,
    /// b6: per-corner accuracy against the tight ngspice reference
    pub corners: Option<TableError>,
    pub accuracy_status: String,
    /// median e2e of this contender / median e2e of the RustSpice reference contender (>1: RustSpice faster); null
    /// unless both contenders completed every repetition
    pub e2e_ratio_vs_rs: Option<f64>,
    /// same for the analysis-only time (RustSpice side excludes its compile time)
    pub analysis_ratio_vs_rs: Option<f64>,
}

impl WorkContender {
    /// Completed every repetition with a valid accuracy figure: the only kind of contender a headline may pick.
    pub fn valid(&self) -> bool {
        self.stats.completed() && self.accuracy_status == "ok"
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct OracleBlock {
    pub spec: String,
    pub status: String,
    pub rs: Option<f64>,
    pub ng_default: Option<f64>,
    pub reference: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkResult {
    pub id: String,
    pub bench: String,
    pub title: String,
    pub detail: String,
    pub size: usize,
    pub legacy_size: bool,
    pub reference: String,
    pub reference_status: String,
    pub rs_baseline: String,
    pub contenders: Vec<WorkContender>,
    pub oracle: Option<OracleBlock>,
    pub reps: usize,
    pub noisy: Vec<String>,
    pub loadavg_start: Option<f64>,
    pub loadavg_end: Option<f64>,
    /// b5: vectors in the ngspice `.sens` rawfile (it writes the sensitivities of the output to every device
    /// parameter, not only the requested resistor values); null when no `.sens` run produced a rawfile
    pub sens_vectors: Option<usize>,
    pub notes: Vec<String>,
}

fn finish_work(
    measured: &[Measured],
    baseline: &str,
    timing: bool,
    mut acc: impl FnMut(&Measured) -> (Option<GradError>, Option<TableError>, String),
) -> (Vec<WorkContender>, usize) {
    let stats: Vec<ContenderStats> = measured.iter().map(stats_of).collect();
    let base = find(&stats, baseline).cloned();
    let reps = stats.iter().map(|s| s.n).max().unwrap_or(0);
    let out = measured
        .iter()
        .zip(stats)
        .map(|(m, s)| {
            let (g, t, status) = acc(m);
            // a partial sample (a repetition failed) never produces a ratio
            let base_ok = base.as_ref().filter(|b| b.completed());
            let both = s.completed() && base_ok.is_some();
            let e2e_ratio = match (&s.e2e_us, base_ok.and_then(|b| b.e2e_us.as_ref())) {
                (Some(a), Some(b)) if timing && both && b.median > 0.0 => Some(a.median / b.median),
                _ => None,
            };
            let analysis_ratio =
                match (&s.analysis_us, base_ok.and_then(|b| b.analysis_us.as_ref())) {
                    (Some(a), Some(b)) if timing && both && b.median > 0.0 => {
                        Some(a.median / b.median)
                    }
                    _ => None,
                };
            WorkContender {
                stats: s,
                gradient: g,
                corners: t,
                accuracy_status: status,
                e2e_ratio_vs_rs: e2e_ratio,
                analysis_ratio_vs_rs: analysis_ratio,
            }
        })
        .collect();
    (out, reps)
}

// ---------------------------------------------------------------------------------------------------------
// b5: sensitivity
// ---------------------------------------------------------------------------------------------------------

/// Central-difference perturbations `(R(1+h), R(1-h))` of every resistor (exactly representable decimal text).
pub fn fd_perturbations(net: &LadderNet, h_rel: f64) -> Vec<(f64, f64)> {
    net.res
        .iter()
        .map(|r| (r.3 * (1.0 + h_rel), r.3 * (1.0 - h_rel)))
        .collect()
}

/// One ngspice process that evaluates every perturbed circuit through a `.control` alter loop.
pub fn fd_control_deck(net: &LadderNet, rsh: f64, target: usize, perts: &[(f64, f64)]) -> String {
    let mut t = String::from(".control\nset numdgt=16\n");
    for ((name, _, _, nominal), (rp, rm)) in net.res.iter().zip(perts) {
        let _ = writeln!(t, "alter {name} = {}\nop\nprint v({target})", num(*rp));
        let _ = writeln!(t, "alter {name} = {}\nop\nprint v({target})", num(*rm));
        let _ = writeln!(t, "alter {name} = {}", num(*nominal));
    }
    t.push_str(".endc\n");
    ladder_deck_with(net.stages, rsh, &t, None)
}

/// The sensitivity plot of an ngspice `.sens` rawfile.
pub fn sensitivity_plot(plots: &[Plot]) -> Option<&Plot> {
    plots
        .iter()
        .rev()
        .find(|p| p.plotname.to_ascii_lowercase().starts_with("sensitivity"))
}

/// Gradients from the `.sens` rawfile of ngspice: the vector `v(<resistor>)` holds `dv(out)/dR`.
pub fn sens_gradients(plots: &[Plot], names: &[String]) -> Result<Vec<f64>, String> {
    let plot = sensitivity_plot(plots).ok_or("no sensitivity plot in the rawfile")?;
    names
        .iter()
        .map(|n| {
            plot.col(&format!("v({})", n.to_ascii_lowercase()))
                .and_then(|c| c.first().copied())
                .ok_or_else(|| format!("no sensitivity vector for `{n}`"))
        })
        .collect()
}

/// Run one b5 workload (a ladder with `stages` series/shunt pairs, all `2*stages` resistors as parameters).
pub fn run_sensitivity(ctx: &Ctx, stages: usize, legacy: bool) -> WorkResult {
    let p = 2 * stages;
    let target = stages + 1;
    let mut res = WorkResult {
        id: format!("b5_sens_{p}"),
        bench: "b5".to_string(),
        title: format!("{p}-parameter sensitivity of v({target}) on a {stages}-stage ladder"),
        detail: format!("{p} resistor gradients (R of every resistor), DC operating point"),
        size: p,
        legacy_size: legacy,
        rs_baseline: "rustspice-adjoint".to_string(),
        loadavg_start: loadavg(),
        ..Default::default()
    };
    let net = ladder_net(stages, R_SH_LEGACY);
    let names: Vec<String> = net.res.iter().map(|r| r.0.clone()).collect();
    let reference = match net.gradients(target) {
        Ok(g) => g,
        Err(e) => {
            res.reference_status = format!("n/a: {e}");
            res.notes
                .push("dense reference failed; accuracy is n/a".to_string());
            Vec::new()
        }
    };
    res.reference = "dense f64 MNA, forward + transposed solve with compensated residual (independent of both simulators)".to_string();
    if !reference.is_empty() {
        res.reference_status = "ok".to_string();
    }
    let dir = ctx.tmp.path().join(&res.id);
    if let Err(e) = fs::create_dir_all(&dir) {
        res.notes.push(format!("setup failed: {e}"));
        return res;
    }
    let setup = (|| -> Result<(), String> {
        write_text(&dir.join("base.sp"), &ladder_deck(stages, R_SH_LEGACY))?;
        let sens = ladder_deck_with(stages, R_SH_LEGACY, &format!(".sens v({target})"), None);
        write_text(&dir.join("sens.sp"), &sens)?;
        write_text(
            &dir.join("sens_klu.sp"),
            &meta::inject_options(&sens, "klu"),
        )?;
        let perts = fd_perturbations(&net, FD_H_CENTRAL);
        let ctl = fd_control_deck(&net, R_SH_LEGACY, target, &perts);
        write_text(&dir.join("fdloop.sp"), &ctl)?;
        write_text(
            &dir.join("fdloop_klu.sp"),
            &meta::inject_options(&ctl, "klu"),
        )?;
        for (i, (rp, rm)) in perts.iter().enumerate() {
            let dp = ladder_deck_with(stages, R_SH_LEGACY, ".op", Some((i, *rp)));
            let dm = ladder_deck_with(stages, R_SH_LEGACY, ".op", Some((i, *rm)));
            write_text(&dir.join(format!("fdc_{i}_p.sp")), &dp)?;
            write_text(&dir.join(format!("fdc_{i}_m.sp")), &dm)?;
        }
        let fwd = fd_perturbations(&net, FD_H_FORWARD);
        write_text(&dir.join("fdf_base.sp"), &ladder_deck(stages, R_SH_LEGACY))?;
        for (i, (rp, _)) in fwd.iter().enumerate() {
            let d = ladder_deck_with(stages, R_SH_LEGACY, ".op", Some((i, *rp)));
            write_text(&dir.join(format!("fdf_{i}.sp")), &d)?;
        }
        Ok(())
    })();
    if let Err(e) = setup {
        res.notes.push(format!("setup failed: {e}"));
        return res;
    }
    let perts = fd_perturbations(&net, FD_H_CENTRAL);
    let fwd = fd_perturbations(&net, FD_H_FORWARD);
    let tnode = target.to_string();
    let vname = format!("v({target})");

    let sens_vectors = Arc::new(AtomicUsize::new(0));
    let mut cs: Vec<Contender> = Vec::new();
    // --- RustSpice
    {
        let (dir, names, tnode) = (dir.clone(), names.clone(), tnode.clone());
        cs.push(Contender {
            name: "rustspice-adjoint".to_string(),
            note: "adjoint: 1 forward + 1 transposed solve, all parameters".to_string(),
            launches: 1,
            threads: 1,
            heavy: false,
            run: Box::new(move |want| {
                let job = WorkerJob {
                    kind: "adjoint".to_string(),
                    deck: dir.join("base.sp"),
                    out: dir.join("adj.json"),
                    target: Some(tnode.clone()),
                    ..Default::default()
                };
                let (child, out) = run_worker(ctx, &dir, "adj", &job, &[], &ctx.lim)?;
                let o = if want {
                    Some(Output::Vector(order_by_names(
                        &names,
                        &out.names,
                        &out.values,
                    )?))
                } else {
                    None
                };
                Ok(worker_sample(&child, &out, o))
            }),
        });
    }
    {
        let (dir, names, tnode) = (dir.clone(), names.clone(), tnode.clone());
        cs.push(Contender {
            name: "rustspice-fd".to_string(),
            note: format!(
                "central FD loop in one RustSpice process ({} operating points)",
                2 * p
            ),
            launches: 1,
            threads: 1,
            heavy: false,
            run: Box::new(move |want| {
                let job = WorkerJob {
                    kind: "fd".to_string(),
                    deck: dir.join("base.sp"),
                    out: dir.join("fd.json"),
                    target: Some(tnode.clone()),
                    h_rel: Some(FD_H_CENTRAL),
                    ..Default::default()
                };
                let (child, out) = run_worker(ctx, &dir, "fd", &job, &[], &ctx.lim)?;
                let o = if want {
                    Some(Output::Vector(order_by_names(
                        &names,
                        &out.names,
                        &out.values,
                    )?))
                } else {
                    None
                };
                Ok(worker_sample(&child, &out, o))
            }),
        });
    }
    if ctx.ng_ok {
        for (solver, deck, tag) in [
            (NgSolver::Sparse, "sens.sp", "sens_s"),
            (NgSolver::Klu, "sens_klu.sp", "sens_k"),
        ] {
            let (dir, names) = (dir.clone(), names.clone());
            let nvec = Arc::clone(&sens_vectors);
            cs.push(Contender {
                name: format!("{NG_SENS_PREFIX}{}", solver.as_str()),
                note: format!(
                    ".sens v({target}), {} solver (one process; computes and writes the sensitivities to every device parameter)",
                    solver.as_str()
                ),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(move |want| {
                    let raw = dir.join(format!("{tag}.raw"));
                    let mut s = ng_run_sample(ctx, &dir.join(deck), &raw, solver, &ctx.lim, want)?;
                    if let Some(Output::Plots(p)) = s.output.take() {
                        if let Some(sp) = sensitivity_plot(&p) {
                            nvec.store(sp.vars.len(), Ordering::Relaxed);
                        }
                        s.output = Some(Output::Vector(sens_gradients(&p, &names)?));
                    } else if want {
                        return Err("no sensitivity rawfile".to_string());
                    }
                    Ok(s)
                }),
            });
        }
        for (solver, deck) in [
            (NgSolver::Sparse, "fdloop.sp"),
            (NgSolver::Klu, "fdloop_klu.sp"),
        ] {
            let (dir, perts, vname) = (dir.clone(), perts.clone(), vname.clone());
            cs.push(Contender {
                name: format!("ngspice-fd-loop-{}", solver.as_str()),
                note: format!(
                    "central FD, ONE ngspice process, .control alter loop ({} ops), {}",
                    2 * p,
                    solver.as_str()
                ),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(move |want| {
                    let mut cmd = Command::new(&ctx.ng);
                    cmd.arg("-b").arg(dir.join(deck));
                    let child = run_guarded(&mut cmd, &ctx.lim).map_err(spawn_msg)?;
                    let log = ngspice::parse_log(&format!("{}\n{}", child.stdout, child.stderr));
                    if child.exit != Exit::Code(0)
                        || !log.failures.is_empty()
                        || child.output_truncated
                    {
                        return Err(log
                            .failures
                            .first()
                            .cloned()
                            .unwrap_or_else(|| exit_msg(&child)));
                    }
                    let output = if want {
                        let v = parse_prints(&child.stdout, &vname);
                        if v.len() != 2 * perts.len() {
                            return Err(format!(
                                "{} values printed, expected {}",
                                v.len(),
                                2 * perts.len()
                            ));
                        }
                        let g: Vec<f64> = perts
                            .iter()
                            .enumerate()
                            .map(|(i, (rp, rm))| (v[2 * i] - v[2 * i + 1]) / (rp - rm))
                            .collect();
                        Some(Output::Vector(g))
                    } else {
                        None
                    };
                    Ok(Sample {
                        e2e_us: child.wall_us(),
                        analysis_us: None,
                        rss_kib: parse_ng_size_kib(&child.stdout),
                        info: log.solver.map(|s| s.as_str().to_string()),
                        output,
                        ..Default::default()
                    })
                }),
            });
        }
        // N launches: sequential decks, one process each
        {
            let (dir, perts) = (dir.clone(), perts.clone());
            cs.push(Contender {
                name: "ngspice-fd-launches-central".to_string(),
                note: format!(
                    "central FD, {} separate ngspice launches (sparse), summed wall",
                    2 * p
                ),
                launches: 2 * p,
                threads: 1,
                heavy: true,
                run: Box::new(move |want| {
                    let mut wall = 0.0;
                    let mut rss: Option<u64> = None;
                    let mut vals = Vec::new();
                    for (i, pert) in perts.iter().enumerate() {
                        let mut pair = [0.0f64; 2];
                        for (j, tag) in ["p", "m"].iter().enumerate() {
                            let deck = dir.join(format!("fdc_{i}_{tag}.sp"));
                            let raw = dir.join(format!("fdc_{i}_{tag}.raw"));
                            let s =
                                ng_run_sample(ctx, &deck, &raw, NgSolver::Sparse, &ctx.lim, want)?;
                            wall += s.e2e_us;
                            rss = rss.max(s.rss_kib);
                            if let Some(Output::Plots(pl)) = s.output {
                                pair[j] = op_value(&pl, &format!("v({})", target))?;
                            }
                        }
                        vals.push((pair[0] - pair[1]) / (pert.0 - pert.1));
                    }
                    Ok(Sample {
                        e2e_us: wall,
                        rss_kib: rss,
                        output: want.then_some(Output::Vector(vals)),
                        ..Default::default()
                    })
                }),
            });
        }
        {
            let (dir, fwd, net_r) = (dir.clone(), fwd.clone(), net.nominal());
            cs.push(Contender {
                name: "ngspice-fd-launches-forward".to_string(),
                note: format!(
                    "forward FD, {} separate ngspice launches (sparse), summed wall",
                    p + 1
                ),
                launches: p + 1,
                threads: 1,
                heavy: true,
                run: Box::new(move |want| {
                    let mut wall = 0.0;
                    let mut rss: Option<u64> = None;
                    let mut base = 0.0;
                    let mut vals = Vec::new();
                    for i in 0..=fwd.len() {
                        let name = if i == 0 {
                            "fdf_base".to_string()
                        } else {
                            format!("fdf_{}", i - 1)
                        };
                        let s = ng_run_sample(
                            ctx,
                            &dir.join(format!("{name}.sp")),
                            &dir.join(format!("{name}.raw")),
                            NgSolver::Sparse,
                            &ctx.lim,
                            want,
                        )?;
                        wall += s.e2e_us;
                        rss = rss.max(s.rss_kib);
                        if let Some(Output::Plots(pl)) = s.output {
                            let v = op_value(&pl, &format!("v({})", target))?;
                            if i == 0 {
                                base = v;
                            } else {
                                vals.push((v - base) / (fwd[i - 1].0 - net_r[i - 1]));
                            }
                        }
                    }
                    Ok(Sample {
                        e2e_us: wall,
                        rss_kib: rss,
                        output: want.then_some(Output::Vector(vals)),
                        ..Default::default()
                    })
                }),
            });
        }
    }
    let measured = measure(&mut cs, ctx.reps, ctx.timing);
    drop(cs);
    let (contenders, reps) = finish_work(&measured, "rustspice-adjoint", ctx.timing, |m| {
        let Some(Output::Vector(g)) = &m.output else {
            return (
                None,
                None,
                if m.ok() {
                    "n/a: no output".to_string()
                } else {
                    m.status.clone()
                },
            );
        };
        if reference.is_empty() {
            return (None, None, "n/a: no reference".to_string());
        }
        match gradient_error(g, &reference) {
            Ok(e) => (Some(e), None, "ok".to_string()),
            Err(e) => (None, None, format!("n/a: {e}")),
        }
    });
    res.contenders = contenders;
    res.reps = reps;
    res.noisy = noisy_names(
        &res.contenders
            .iter()
            .map(|c| c.stats.clone())
            .collect::<Vec<_>>(),
    );
    let nv = sens_vectors.load(Ordering::Relaxed);
    res.sens_vectors = (nv > 0).then_some(nv);
    if let Some(nv) = res.sens_vectors {
        res.notes.push(format!(
            "ngspice `.sens` computes and writes {nv} sensitivity vectors for the {p} requested resistor gradients ({:.1} per requested gradient: the sensitivities to every device parameter, not only R); only the {p} requested ones are compared. RustSpice's adjoint returns exactly the {p} requested gradients, so the `.sens` rows are not equal output work; the finite-difference rows deliver the same {p} gradients and nothing more.",
            nv as f64 / p as f64
        ));
    }
    res.notes.push(format!(
        "finite-difference steps: central h = {FD_H_CENTRAL:e} (relative), forward h = {FD_H_FORWARD:e}; ngspice FD decks use default tolerances (linear circuit: one exact Newton step)"
    ));
    res.notes.push(DENSE_REFERENCE_NOTE.to_string());
    res.loadavg_end = loadavg();
    res
}

/// Own error of the b5 dense f64 reference on the 100-stage ladder (scale-normalised, max |g - exact| / max |exact|),
/// measured offline against a 50-digit arithmetic adjoint (mpmath: 1.2e-13; RustSpice's adjoint 9.4e-14 against the
/// same exact gradient). Pinned by the unit test `dense_reference_error_against_arbitrary_precision_gradients`.
const DENSE_REF_OWN_ERROR: &str = "about 1.2e-13";
const DENSE_REFERENCE_NOTE: &str = "the dense f64 reference has its own error: about 1.2e-13 scale-normalised on the 100-stage ladder (measured against a 50-digit arithmetic adjoint), which is larger than the RustSpice adjoint's own error against the exact gradient (about 9.4e-14) and than the RustSpice-to-reference distance printed in the gradient-error column; differences of that size or below are not resolved by this table.";

/// First value of `name` in the last operating-point plot.
fn op_value(plots: &[Plot], name: &str) -> Result<f64, String> {
    crate::bench::rawfile::last_plot(plots, PlotKind::Op)
        .and_then(|p| p.col(name))
        .and_then(|c| c.first().copied())
        .ok_or_else(|| format!("no `{name}` in the operating-point plot"))
}

// ---------------------------------------------------------------------------------------------------------
// b6: batch corners
// ---------------------------------------------------------------------------------------------------------

/// Number of diodes of the batch circuit (legacy b6).
const BATCH_DIODES: usize = 32;

/// `(labels, rows)` of the `v(...)` vectors of a DC-sweep plot (one row per sweep point).
pub fn table_from_dc(plots: &[Plot]) -> Result<(Vec<String>, Vec<Vec<f64>>), String> {
    let p = crate::bench::rawfile::last_plot(plots, PlotKind::Dc).ok_or("no dc plot")?;
    table_from_plot(p, 1)
}

fn table_from_plot(p: &Plot, skip: usize) -> Result<(Vec<String>, Vec<Vec<f64>>), String> {
    let idx: Vec<usize> = p
        .vars
        .iter()
        .enumerate()
        .skip(skip)
        .filter(|(_, v)| v.name.starts_with("v("))
        .map(|(i, _)| i)
        .collect();
    if idx.is_empty() {
        return Err("plot has no v() vectors".to_string());
    }
    let labels = idx.iter().map(|&i| p.vars[i].name.clone()).collect();
    let rows = (0..p.npoints)
        .map(|k| idx.iter().map(|&i| p.cols[i][k]).collect())
        .collect();
    Ok((labels, rows))
}

/// One row per operating-point plot, columns aligned to `labels`.
pub fn table_from_ops(plots: &[Plot], labels: &[String]) -> Result<Vec<Vec<f64>>, String> {
    plots
        .iter()
        .filter(|p| p.kind() == Some(PlotKind::Op))
        .map(|p| {
            labels
                .iter()
                .map(|l| {
                    p.col(l)
                        .and_then(|c| c.first().copied())
                        .ok_or_else(|| format!("no `{l}` in an operating-point plot"))
                })
                .collect()
        })
        .collect()
}

/// `.control` deck that visits every corner in one ngspice process and appends each operating point to `raw`.
pub fn corner_control_deck(n_diodes: usize, values: &[f64], raw: &Path) -> String {
    let mut t = String::from(".control\nset appendwrite\nset filetype=binary\n");
    for v in values {
        let _ = writeln!(t, "alter v1 = {}\nop\nwrite {} all", num(*v), raw.display());
    }
    t.push_str(".endc\n");
    diode_cascade_deck_with(n_diodes, 5.0, &t)
}

fn dc_plot_from_table(axis: &[f64], labels: &[String], rows: &[Vec<f64>]) -> Plot {
    use super::rawfile::Var;
    let mut vars = vec![Var {
        name: "v(v-sweep)".to_string(),
        kind: "voltage".to_string(),
    }];
    let mut cols = vec![axis.to_vec()];
    for (j, l) in labels.iter().enumerate() {
        vars.push(Var {
            name: l.clone(),
            kind: "voltage".to_string(),
        });
        cols.push(rows.iter().map(|r| r[j]).collect());
    }
    Plot {
        title: "batch corners".to_string(),
        plotname: PlotKind::Dc.plotname().to_string(),
        vars,
        npoints: rows.len(),
        cols,
    }
}

/// Table sample of N sequential or parallel ngspice launches (`c_<i>.sp` / `c_<i>.raw`).
fn launch_corners(
    ctx: &Ctx,
    dir: &Path,
    n: usize,
    parallel: bool,
    want: bool,
    labels: &[String],
) -> Result<Sample, String> {
    let one = |i: usize, lim: &Limits| -> Result<Sample, String> {
        ng_run_sample(
            ctx,
            &dir.join(format!("c_{i}.sp")),
            &dir.join(format!("c_{i}.raw")),
            NgSolver::Sparse,
            lim,
            false,
        )
    };
    let (e2e, rss) = if parallel {
        let (wall, err, rss) =
            run_parallel(n, ctx.threads, |i| one(i, &ctx.lim_free).map(|s| s.rss_kib));
        if let Some(e) = err {
            return Err(e);
        }
        (wall.as_secs_f64() * 1e6, rss)
    } else {
        let mut wall = 0.0;
        let mut rss: Option<u64> = None;
        for i in 0..n {
            let s = one(i, &ctx.lim)?;
            wall += s.e2e_us;
            rss = rss.max(s.rss_kib);
        }
        (wall, rss)
    };
    let output = if want {
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            let plots = crate::bench::rawfile::read_file(&dir.join(format!("c_{i}.raw")))
                .map_err(|e| e.to_string())?;
            rows.extend(table_from_ops(&plots, labels)?);
        }
        Some(Output::Table {
            labels: labels.to_vec(),
            rows,
        })
    } else {
        None
    };
    Ok(Sample {
        e2e_us: e2e,
        rss_kib: rss,
        output,
        ..Default::default()
    })
}

/// Run one b6 workload: `corners` supply corners of the 32-diode cascade.
pub fn run_batch(ctx: &Ctx, corners: usize, legacy: bool) -> WorkResult {
    let vals = corner_values(corners);
    let mut res = WorkResult {
        id: format!("b6_batch_{corners}"),
        bench: "b6".to_string(),
        title: format!("{corners}-corner batch of the {BATCH_DIODES}-diode cascade (.op per corner)"),
        detail: format!(
            "corner c sets V1 = {CORNER_V0} + {CORNER_DV}*c V, c = 0..{}; every contender solves the same {corners} operating points",
            corners - 1
        ),
        size: corners,
        legacy_size: legacy,
        rs_baseline: "rustspice-batch".to_string(),
        loadavg_start: loadavg(),
        ..Default::default()
    };
    let dir = ctx.tmp.path().join(&res.id);
    if let Err(e) = fs::create_dir_all(&dir) {
        res.notes.push(format!("setup failed: {e}"));
        return res;
    }
    let sweep_deck = diode_cascade_deck_with(BATCH_DIODES, 5.0, &corner_sweep_card(corners));
    let ctl_s_raw = dir.join("ctl_s.raw");
    let ctl_k_raw = dir.join("ctl_k.raw");
    let setup = (|| -> Result<(), String> {
        write_text(&dir.join("base.sp"), &diode_cascade_deck(BATCH_DIODES))?;
        write_text(&dir.join("sweep.sp"), &sweep_deck)?;
        write_text(
            &dir.join("sweep_klu.sp"),
            &meta::inject_options(&sweep_deck, "klu"),
        )?;
        for (i, v) in vals.iter().enumerate() {
            write_text(
                &dir.join(format!("c_{i}.sp")),
                &diode_cascade_deck_with(BATCH_DIODES, *v, ".op"),
            )?;
        }
        let ctl_s = corner_control_deck(BATCH_DIODES, &vals, &ctl_s_raw);
        write_text(&dir.join("ctl_s.sp"), &ctl_s)?;
        let ctl_k = corner_control_deck(BATCH_DIODES, &vals, &ctl_k_raw);
        write_text(&dir.join("ctl_k.sp"), &meta::inject_options(&ctl_k, "klu"))
    })();
    if let Err(e) = setup {
        res.notes.push(format!("setup failed: {e}"));
        return res;
    }
    // ---- reference: tight ngspice .dc sweep over exactly the corner values
    res.reference =
        format!("ngspice .dc sweep over the same {corners} values, SPARSE, {TIGHT_OPTIONS}");
    let rf = if ctx.ng_ok {
        Some(make_reference(ctx, &sweep_deck, PlotKind::Dc, None, &dir))
    } else {
        None
    };
    let ref_table = match &rf {
        Some(r) => match &r.plots {
            Ok(p) => table_from_dc(p),
            Err(e) => Err(e.clone()),
        },
        None => Err("ngspice is not installed (unverified)".to_string()),
    };
    res.reference_status = match &ref_table {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("n/a: {e}"),
    };
    let ref_labels: Vec<String> = ref_table.as_ref().map(|t| t.0.clone()).unwrap_or_default();

    let mut cs: Vec<Contender> = Vec::new();
    for (name, note, threads, env_one) in [
        (
            "rustspice-batch",
            "run_batch_dc_op: rayon batch of all corners, default thread pool",
            ctx.threads,
            false,
        ),
        (
            "rustspice-batch-1thread",
            "run_batch_dc_op with RAYON_NUM_THREADS=1",
            1,
            true,
        ),
    ] {
        let (dir, vals) = (dir.clone(), vals.clone());
        cs.push(Contender {
            name: name.to_string(),
            note: note.to_string(),
            launches: 1,
            threads,
            heavy: false,
            run: Box::new(move |want| {
                let job = WorkerJob {
                    kind: "batch".to_string(),
                    deck: dir.join("base.sp"),
                    out: dir.join("batch.json"),
                    values: vals.clone(),
                    ..Default::default()
                };
                let env: Vec<(&str, String)> = if env_one {
                    vec![("RAYON_NUM_THREADS", "1".to_string())]
                } else {
                    Vec::new()
                };
                let lim = if env_one { &ctx.lim } else { &ctx.lim_free };
                let (child, out) = run_worker(ctx, &dir, "batch", &job, &env, lim)?;
                let o = want.then(|| Output::Table {
                    labels: out.names.clone(),
                    rows: out.rows.clone(),
                });
                Ok(worker_sample(&child, &out, o))
            }),
        });
    }
    {
        let (dir, vals) = (dir.clone(), vals.clone());
        cs.push(Contender {
            name: "rustspice-seq-loop".to_string(),
            note: "sequential in-process loop of run_dc_op, one thread".to_string(),
            launches: 1,
            threads: 1,
            heavy: false,
            run: Box::new(move |want| {
                let job = WorkerJob {
                    kind: "seq".to_string(),
                    deck: dir.join("base.sp"),
                    out: dir.join("seq.json"),
                    values: vals.clone(),
                    ..Default::default()
                };
                let (child, out) = run_worker(ctx, &dir, "seq", &job, &[], &ctx.lim)?;
                let o = want.then(|| Output::Table {
                    labels: out.names.clone(),
                    rows: out.rows.clone(),
                });
                Ok(worker_sample(&child, &out, o))
            }),
        });
    }
    {
        let dir = dir.clone();
        cs.push(Contender {
            name: "rustspice-dc-sweep".to_string(),
            note: "rustspice run on a .dc sweep over the same values (warm-started points)"
                .to_string(),
            launches: 1,
            threads: 1,
            heavy: false,
            run: Box::new(move |want| {
                rs_run_sample(ctx, &dir.join("sweep.sp"), &dir, "sweep", &ctx.lim, want)
            }),
        });
    }
    if ctx.ng_ok {
        {
            let (dir, labels) = (dir.clone(), ref_labels.clone());
            cs.push(Contender {
                name: "ngspice-seq-launches".to_string(),
                note: format!("{corners} sequential ngspice launches (.op deck per corner), sparse, summed wall"),
                launches: corners,
                threads: 1,
                heavy: true,
                run: Box::new(move |want| launch_corners(ctx, &dir, corners, false, want, &labels)),
            });
        }
        {
            let (dir, labels) = (dir.clone(), ref_labels.clone());
            let p = ctx.threads.min(corners).max(1);
            cs.push(Contender {
                name: "ngspice-par-launches".to_string(),
                note: format!("{corners} ngspice launches on {p} concurrent workers, sparse, wall of the whole set"),
                launches: corners,
                threads: p,
                heavy: true,
                run: Box::new(move |want| launch_corners(ctx, &dir, corners, true, want, &labels)),
            });
        }
        for (solver, deck, raw) in [
            (NgSolver::Sparse, "ctl_s.sp", ctl_s_raw.clone()),
            (NgSolver::Klu, "ctl_k.sp", ctl_k_raw.clone()),
        ] {
            let dir = dir.clone();
            let labels = ref_labels.clone();
            cs.push(Contender {
                name: format!("ngspice-control-loop-{}", solver.as_str()),
                note: format!(
                    "ONE ngspice process, .control alter/op loop over the corners, {}",
                    solver.as_str()
                ),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(move |want| {
                    let _ = fs::remove_file(&raw);
                    let mut cmd = Command::new(&ctx.ng);
                    cmd.arg("-b").arg(dir.join(deck));
                    let child = run_guarded(&mut cmd, &ctx.lim).map_err(spawn_msg)?;
                    let log = ngspice::parse_log(&format!("{}\n{}", child.stdout, child.stderr));
                    if child.exit != Exit::Code(0) || !log.failures.is_empty() {
                        return Err(log
                            .failures
                            .first()
                            .cloned()
                            .unwrap_or_else(|| exit_msg(&child)));
                    }
                    let output = if want {
                        let plots =
                            crate::bench::rawfile::read_file(&raw).map_err(|e| e.to_string())?;
                        Some(Output::Table {
                            labels: labels.clone(),
                            rows: table_from_ops(&plots, &labels)?,
                        })
                    } else {
                        None
                    };
                    Ok(Sample {
                        e2e_us: child.wall_us(),
                        rss_kib: parse_ng_size_kib(&child.stdout),
                        info: log.solver.map(|s| s.as_str().to_string()),
                        output,
                        ..Default::default()
                    })
                }),
            });
        }
        for (solver, deck, tag) in [
            (NgSolver::Sparse, "sweep.sp", "sw_s"),
            (NgSolver::Klu, "sweep_klu.sp", "sw_k"),
        ] {
            let dir = dir.clone();
            cs.push(Contender {
                name: format!("ngspice-dc-sweep-{}", solver.as_str()),
                note: format!(
                    "ONE ngspice process, .dc sweep over the same values, {}",
                    solver.as_str()
                ),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(move |want| {
                    ng_run_sample(
                        ctx,
                        &dir.join(deck),
                        &dir.join(format!("{tag}.raw")),
                        solver,
                        &ctx.lim,
                        want,
                    )
                }),
            });
        }
    }
    let measured = measure(&mut cs, ctx.reps, ctx.timing);
    drop(cs);
    let table_of = |m: &Measured| -> Result<(Vec<String>, Vec<Vec<f64>>), String> {
        match m.output.as_ref() {
            Some(Output::Table { labels, rows }) => Ok((labels.clone(), rows.clone())),
            Some(Output::Plots(p)) => table_from_dc(p),
            _ => Err("no output".to_string()),
        }
    };
    let (contenders, reps) = finish_work(&measured, "rustspice-batch", ctx.timing, |m| {
        let Ok((rl, rr)) = &ref_table else {
            return (None, None, "n/a: no reference".to_string());
        };
        if !m.ok() {
            return (None, None, m.status.clone());
        }
        match table_of(m).and_then(|(l, r)| table_error(&l, &r, rl, rr)) {
            Ok(t) => (None, Some(t), "ok".to_string()),
            Err(e) => (None, None, format!("n/a: {e}")),
        }
    });
    res.contenders = contenders;
    res.reps = reps;
    res.noisy = noisy_names(
        &res.contenders
            .iter()
            .map(|c| c.stats.clone())
            .collect::<Vec<_>>(),
    );
    // ---- independent oracle (dense f64 Newton, oracle.rs) on three nodes, judged through the engine's compare()
    if let (Some(rfr), Some(rs_m)) = (
        rf.as_ref().and_then(|r| r.plots.as_ref().ok()),
        measured
            .iter()
            .find(|m| m.name == "rustspice-batch" && m.ok()),
    ) {
        if let Ok((labels, rows)) = table_of(rs_m) {
            let rs_plot = dc_plot_from_table(&vals, &labels, &rows);
            let asis = plots_of(
                measured
                    .iter()
                    .find(|m| m.name == "ngspice-dc-sweep-sparse"),
            );
            let n = BATCH_DIODES;
            let probes: Vec<String> = [n + 2, n + 1 + n / 2, 2 * n + 1]
                .iter()
                .map(|k| format!("v({k})"))
                .collect();
            let mo = compare_meta(&res.id, 1e-6, probes, Some("dc_nonlinear"), false);
            let rs_arr = [rs_plot];
            let cr = compare(&CompareInput {
                meta: &mo,
                deck: &sweep_deck,
                rs: &rs_arr,
                reference: rfr,
                asis,
            });
            let cr_r = compare(&CompareInput {
                meta: &mo,
                deck: &sweep_deck,
                rs: rfr,
                reference: rfr,
                asis: None,
            });
            res.oracle = cr.oracle.as_ref().map(|o| OracleBlock {
                spec: o.spec.clone(),
                status: match &o.reason {
                    Some(r) => format!("{}: {r}", o.status),
                    None => o.status.clone(),
                },
                rs: o.rs_max_err,
                ng_default: o.ng_max_err,
                reference: cr_r.oracle.as_ref().and_then(|x| x.rs_max_err),
            });
        }
    }
    res.notes.push(
        "corner accuracy = max |v - v_ref| over all nodes and corners (volts) against the tight ngspice sweep".to_string(),
    );
    res.loadavg_end = loadavg();
    res
}

// ---------------------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct HostInfo {
    pub cpu: String,
    /// Logical CPUs the benchmark may use (`available_parallelism`: affinity / cgroup limited)
    pub threads: usize,
    /// Logical CPUs of the machine (`processor` entries of /proc/cpuinfo); null when unreadable
    pub hardware_threads: Option<usize>,
    pub loadavg_start: Option<f64>,
    pub loadavg_end: Option<f64>,
    pub pin_cpu: Option<usize>,
    pub tmp_on_tmpfs: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct NgInfo {
    pub version: Option<String>,
    pub path: String,
    pub installed: bool,
    pub solvers_timed: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RsInfo {
    pub version: String,
    /// Short HEAD of the git tree this binary was built from (`CARGO_MANIFEST_DIR`), read when the benchmark runs;
    /// null when that tree is not available or is not a git checkout
    pub git: Option<String>,
    /// The source tree has uncommitted changes (null when unknown)
    pub git_dirty: Option<bool>,
    /// Directory the git state was read from
    pub source_tree: String,
    pub build_profile: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScopeInfo {
    pub temperature_c: f64,
    pub ngspice_asis_tolerances: String,
    pub ngspice_reference_op_dc: String,
    pub ngspice_reference_tran: String,
    pub rustspice_tolerances: String,
    pub step_policy: String,
    pub reps_requested: usize,
    pub timing_enabled: bool,
    pub tran_floor_v: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScalingRow {
    pub family: String,
    pub metric: String,
    pub contender: String,
    pub fit: Fit,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesPoint {
    pub tstep_s: f64,
    pub rs_err: Option<f64>,
    pub ng_err: Option<f64>,
    pub rs_e2e_us: Option<f64>,
    pub ng_e2e_us: Option<f64>,
    /// RustSpice `analysis_us` (excludes its compile time) / ngspice `Total analysis time`
    pub rs_analysis_us: Option<f64>,
    pub ng_analysis_us: Option<f64>,
    /// RustSpice `compile_us + analysis_us`
    pub rs_compile_analysis_us: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeriesResult {
    pub id: String,
    pub title: String,
    pub points: Vec<SeriesPoint>,
    pub equal_error_e2e: Option<EqualError>,
    /// analysis-only times (RustSpice excludes its compile time): NOT like-for-like
    pub equal_error_analysis: Option<EqualError>,
    /// ngspice `Total analysis time` against RustSpice `compile_us + analysis_us`
    pub equal_error_like_for_like: Option<EqualError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: u32,
    pub tool: String,
    pub elapsed_s: f64,
    pub parent_peak_rss_kib: Option<u64>,
    pub host: HostInfo,
    pub ngspice: NgInfo,
    pub rustspice: RsInfo,
    pub scope: ScopeInfo,
    pub floors: Floors,
    pub cases: Vec<CaseResult>,
    pub series: Vec<SeriesResult>,
    pub sensitivity: Vec<WorkResult>,
    pub batch: Vec<WorkResult>,
    pub scaling: Vec<ScalingRow>,
    pub previously_published: Vec<Published>,
}

/// Case family key: the id without its trailing `_<size>`.
pub fn family_of(id: &str) -> &str {
    id.rsplit_once('_').map_or(id, |(f, _)| f)
}

/// Log-log scaling fits (3-point ladders) of the e2e, analysis-only and like-for-like analysis medians against the
/// problem size (contenders that completed every repetition only).
pub fn scaling_rows(cases: &[CaseResult]) -> Vec<ScalingRow> {
    let mut fams: BTreeMap<&str, Vec<&CaseResult>> = BTreeMap::new();
    for c in cases.iter().filter(|c| !c.id.starts_with("b2s_")) {
        fams.entry(family_of(&c.id)).or_default().push(c);
    }
    let mut out = Vec::new();
    for (fam, cs) in fams {
        for name in [RS, NG_SPARSE, NG_KLU] {
            for metric in ["e2e", "analysis", "like_for_like"] {
                let pts: Vec<(f64, f64)> = cs
                    .iter()
                    .filter_map(|c| {
                        let s = c.stat(name).filter(|s| s.completed())?;
                        let m = match metric {
                            "e2e" => &s.e2e_us,
                            // like-for-like: RustSpice compile + analysis against ngspice `Total analysis time`
                            "like_for_like" if name == RS => &s.compile_analysis_us,
                            _ => &s.analysis_us,
                        };
                        Some((c.size as f64, m.as_ref()?.median))
                    })
                    .collect();
                if pts.is_empty() {
                    continue;
                }
                out.push(ScalingRow {
                    family: fam.to_string(),
                    metric: metric.to_string(),
                    contender: name.to_string(),
                    fit: loglog_fit(&pts),
                });
            }
        }
    }
    out
}

/// GitHub-flavoured Markdown table; every row is padded / truncated to the header width.
pub fn md_table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut s = String::new();
    let head: Vec<String> = header.iter().map(|h| md_escape(h)).collect();
    let _ = writeln!(s, "| {} |", head.join(" | "));
    let _ = writeln!(
        s,
        "|{}|",
        header.iter().map(|_| " --- ").collect::<Vec<_>>().join("|")
    );
    for r in rows {
        let mut cells: Vec<String> = r.iter().map(|c| md_escape(c)).collect();
        cells.resize(header.len(), "n/a".to_string());
        let _ = writeln!(s, "| {} |", cells.join(" | "));
    }
    s
}

fn speed_cell(med: &Ratio, min: &Ratio) -> String {
    match med.vs_fastest {
        Some(m) => format!(
            "{} (min {}) vs {}",
            fmt_x(Some(m)),
            fmt_x(min.vs_fastest),
            med.fastest.as_deref().unwrap_or("?")
        ),
        None => "n/a".to_string(),
    }
}

fn rs_solver(c: &CaseResult) -> String {
    c.stat(RS)
        .and_then(|s| s.info.clone())
        .unwrap_or_else(|| "n/a".to_string())
}

fn legacy_case<'a>(r: &'a Report, bench: &str) -> Option<&'a CaseResult> {
    r.cases.iter().find(|c| c.bench == bench && c.legacy_size)
}

fn work_contender<'a>(w: &'a WorkResult, name: &str) -> Option<&'a WorkContender> {
    w.contenders.iter().find(|c| c.stats.name == name)
}

/// Median e2e time of a contender that completed every repetition (a partial sample is not a result).
fn med_of(c: Option<&WorkContender>) -> Option<f64> {
    c.filter(|c| c.stats.completed())
        .and_then(|c| c.stats.e2e_us.as_ref())
        .map(|s| s.median)
}

fn min_of(c: Option<&WorkContender>) -> Option<f64> {
    c.filter(|c| c.stats.completed())
        .and_then(|c| c.stats.e2e_us.as_ref())
        .map(|s| s.min)
}

fn legacy_work(v: &[WorkResult]) -> Option<&WorkResult> {
    v.iter().find(|w| w.legacy_size)
}

/// The fastest contender (median e2e) of a b5 / b6 block whose name starts with `prefix` and is not `skip`ped, among
/// the contenders that completed EVERY repetition with a valid accuracy figure: a contender that failed in a later
/// repetition, or whose accuracy is `n/a`, can never become a headline variant.
fn best_of<'a>(
    w: &'a WorkResult,
    prefix: &str,
    skip: impl Fn(&WorkContender) -> bool,
) -> Option<&'a WorkContender> {
    w.contenders
        .iter()
        .filter(|c| c.stats.name.starts_with(prefix) && c.valid() && !skip(c))
        .filter(|c| c.stats.e2e_us.is_some())
        .min_by(|a, b| {
            let (x, y) = (med_of(Some(a)), med_of(Some(b)));
            x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Fastest valid ngspice variant (may be `.sens`, which computes more than the requested gradients).
fn best_ngspice(w: &WorkResult) -> Option<&WorkContender> {
    best_of(w, "ngspice", |_| false)
}

/// Fastest valid ngspice variant that delivers only the requested gradients (every b5 variant except `.sens`).
fn best_ngspice_requested_only(w: &WorkResult) -> Option<&WorkContender> {
    best_of(w, "ngspice", |c| c.stats.name.starts_with(NG_SENS_PREFIX))
}

/// Fastest valid RustSpice variant.
fn best_rustspice(w: &WorkResult) -> Option<&WorkContender> {
    best_of(w, "rustspice", |_| false)
}

/// A headline contender: complete and with a valid accuracy figure.
fn valid_contender<'a>(w: &'a WorkResult, name: &str) -> Option<&'a WorkContender> {
    work_contender(w, name).filter(|c| c.valid())
}

/// `num / den` of two contender medians (>1: `den` is faster) and of their minima.
fn work_ratio(
    num: Option<&WorkContender>,
    den: Option<&WorkContender>,
) -> (Option<f64>, Option<f64>) {
    let f = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) if b > 0.0 && a.is_finite() && b.is_finite() => Some(a / b),
        _ => None,
    };
    (f(med_of(num), med_of(den)), f(min_of(num), min_of(den)))
}

/// `3.2x (min 2.9x) vs name` for a pair of work contenders.
fn work_speed_cell(num: Option<&WorkContender>, den: Option<&WorkContender>) -> String {
    match (num, work_ratio(num, den)) {
        (Some(n), (Some(m), mn)) => {
            format!("{} (min {}) vs {}", fmt_x(Some(m)), fmt_x(mn), n.stats.name)
        }
        _ => "n/a".to_string(),
    }
}

/// `n` cell: repetitions completed, with the number scheduled when they differ.
fn n_cell(s: Option<&ContenderStats>) -> String {
    match s {
        Some(s) if s.reps_planned == 0 && s.n == 0 => "n/a (not timed)".to_string(),
        Some(s) if s.n == s.reps_planned => s.n.to_string(),
        Some(s) => format!("{} of {}", s.n, s.reps_planned),
        None => "n/a".to_string(),
    }
}

/// RustSpice error at or above this multiple of ngspice's default-tolerance error marks a speedup as measured at
/// unequal accuracy.
pub const UNEQUAL_ACCURACY_FACTOR: f64 = 2.0;

/// `(RustSpice error, ngspice default error, basis)` judged by the independent oracle where one exists, else by the
/// tight reference (whose own error then bounds both figures from below).
pub fn accuracy_pair(a: &Accuracy) -> Option<(f64, f64, &'static str)> {
    if a.oracle_status.as_deref() == Some("ok") {
        if let (Some(r), Some(n)) = (a.rs_vs_oracle, a.ng_vs_oracle) {
            return Some((r, n, "independent oracle"));
        }
    }
    match (a.rs_err, a.ng_err) {
        (Some(r), Some(n)) => Some((r, n, "tight reference")),
        _ => None,
    }
}

/// Qualifier text for a speedup that was measured while RustSpice was clearly the less accurate tool.
pub fn unequal_accuracy_text(a: &Accuracy) -> Option<String> {
    let (rs, ng, basis) = accuracy_pair(a)?;
    if !rs.is_finite() || !ng.is_finite() || rs < UNEQUAL_ACCURACY_FACTOR * ng || rs <= 0.0 {
        return None;
    }
    Some(if ng > 0.0 {
        format!(
            "unequal accuracy: RustSpice error is {} ngspice default's at the same nominal settings ({basis}: {} vs {})",
            fmt_x(Some(rs / ng)),
            fmt_sci(Some(rs)),
            fmt_sci(Some(ng))
        )
    } else {
        format!(
            "unequal accuracy: RustSpice error {} against the {basis}, while the ngspice default run is identical to it",
            fmt_sci(Some(rs))
        )
    })
}

/// Equal-error comparison of the b2 step ladder, for the b2 headline row.
fn b2_equal_error_text(r: &Report) -> Option<String> {
    let s = r.series.iter().find(|s| s.id.starts_with("b2s_"))?;
    Some(match &s.equal_error_e2e {
        None => format!(
            "equal-error table of the step ladder `{}`: n/a (too few usable points)",
            s.id
        ),
        Some(e) if e.status == "ok" => {
            let ratios: Vec<f64> = e.levels.iter().map(|l| l.ratio_b_over_a).collect();
            let lo = ratios.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = ratios.iter().copied().fold(0.0f64, f64::max);
            format!(
                "equal-error end-to-end ratio (ngspice / RustSpice) on the step ladder `{}`: {} .. {} over errors {}",
                s.id,
                fmt_x(Some(lo).filter(|v| v.is_finite())),
                fmt_x(Some(hi).filter(|v| *v > 0.0)),
                e.range.map_or("n/a".to_string(), |(a, b)| format!("{a:.1e} .. {b:.1e}"))
            )
        }
        Some(e) => {
            let rng = |r: Option<(f64, f64)>| {
                r.map_or("n/a".to_string(), |(a, b)| format!("{a:.1e} .. {b:.1e}"))
            };
            let worse = matches!((e.a_range, e.b_range), (Some(a), Some(b)) if a.0 > b.1);
            format!(
                "no equal-error comparison exists on the step ladder `{}` ({}): RustSpice error range {}, ngspice {}{}",
                s.id,
                e.status,
                rng(e.a_range),
                rng(e.b_range),
                if worse {
                    "; RustSpice does not reach ngspice's error in the tested step range"
                } else {
                    ""
                }
            )
        }
    })
}

/// The b1 oracle is the f64 impedance recursion `ladder_voltages`, not the exact solution: its own error is measured
/// against 80-digit arithmetic (mpmath, 5 000 stages: 4.4e-15 V for the legacy 10 kohm ladder, 8.5e-15 V for the flat
/// ladder). Pinned by the unit test `ladder_recursion_error_is_below_the_stated_bound`.
const LADDER_ORACLE_NOTE: &str = "The exact ladder recursion is evaluated in f64, so the oracle column itself carries an absolute error of about 4e-15 V (legacy ladder) to 9e-15 V (flat ladder) at 5 000 stages (measured against 80-digit arithmetic): figures near 1e-13 V are good to a few percent, figures near 1e-10 V to better than 0.01 %.";

/// Zero means "identical to the reference", not zero error.
fn fmt_agree(v: Option<f64>) -> String {
    match v {
        Some(0.0) => "0 (identical to the reference)".to_string(),
        _ => fmt_sci(v),
    }
}

/// Volts with the zero case spelled out: `0 V (identical to the reference)`.
fn fmt_agree_v(v: Option<f64>) -> String {
    match v {
        Some(0.0) => "0 V (identical to the reference)".to_string(),
        Some(_) => format!("{} V", fmt_sci(v)),
        None => "n/a".to_string(),
    }
}

/// Accuracy against the independent oracle (RustSpice / ngspice default), with the tight reference's own error.
fn oracle_cell(a: &Accuracy, tran: bool) -> String {
    let unit = if tran { "norm." } else { "V" };
    match (&a.oracle, a.oracle_status.as_deref(), a.rs_vs_oracle) {
        (Some(_), Some("ok"), Some(rs)) => format!(
            "{} / {} {unit} (tight reference itself: {})",
            fmt_sci(Some(rs)),
            fmt_sci(a.ng_vs_oracle),
            fmt_sci(a.ref_vs_oracle)
        ),
        (Some(o), st, _) => format!("n/a (oracle `{o}`: {})", st.unwrap_or("unavailable")),
        (None, _, _) => match a.ref_selfcheck {
            Some(v) => format!(
                "n/a (no oracle for this circuit; tight reference self-check: {})",
                fmt_sci(Some(v))
            ),
            None => "n/a (no oracle for this circuit)".to_string(),
        },
    }
}

fn flags_cell(noisy: &[String], stats: &[&ContenderStats]) -> String {
    let mut v: Vec<String> = Vec::new();
    if !noisy.is_empty() {
        v.push(format!("noisy: {}", noisy.join(", ")));
    }
    for s in stats.iter().filter(|s| s.status != "ok") {
        v.push(format!("{}: {}", s.name, s.status));
    }
    if v.is_empty() {
        "-".to_string()
    } else {
        v.join("; ")
    }
}

fn case_headline_row(r: &Report, c: &CaseResult, label: &str) -> Vec<String> {
    let rs = c.stat(RS);
    let sp = c.speedups.clone().unwrap_or_default();
    let med_e2e = |s: Option<&ContenderStats>| {
        s.filter(|s| s.completed())
            .and_then(|s| s.e2e_us.as_ref())
            .map(|s| s.median)
    };
    let ng_best = [NG_SPARSE, NG_KLU]
        .iter()
        .filter_map(|n| med_e2e(c.stat(n)))
        .reduce(f64::min);
    let ng_fast = sp
        .e2e_median
        .fastest
        .as_deref()
        .map(|f| format!("ngspice-{f}"))
        .and_then(|n| c.stat(&n));
    let mut speed = speed_cell(&sp.e2e_median, &sp.e2e_min);
    let unequal = unequal_accuracy_text(&c.accuracy);
    if unequal.is_some() && sp.e2e_median.vs_fastest.is_some() {
        speed.push_str("; unequal accuracy");
    }
    let mut qual: Vec<String> = Vec::new();
    qual.extend(unequal);
    if c.bench == "b2" {
        qual.extend(b2_equal_error_text(r));
    }
    let all: Vec<&ContenderStats> = c.contenders.iter().collect();
    vec![
        format!("{label} ({})", c.detail),
        fmt_opt_time(med_e2e(rs)),
        fmt_opt_time(ng_best),
        speed,
        speed_cell(&sp.analysis_median, &sp.analysis_min),
        speed_cell(&sp.like_for_like_median, &sp.like_for_like_min),
        if c.accuracy.status == "ok" {
            oracle_cell(&c.accuracy, c.analysis == "tran")
        } else {
            c.accuracy.status.clone()
        },
        if c.accuracy.status == "ok" {
            format!(
                "{} / {}",
                fmt_agree(c.accuracy.rs_err),
                fmt_agree(c.accuracy.ng_err)
            )
        } else {
            c.accuracy.status.clone()
        },
        format!("{} / {}", n_cell(rs), n_cell(ng_fast)),
        flags_cell(&c.noisy, &all),
        if qual.is_empty() {
            "-".to_string()
        } else {
            qual.join("; ")
        },
    ]
}

fn sens_headline_row(w: &WorkResult) -> Vec<String> {
    let rs = valid_contender(w, "rustspice-adjoint");
    let best = best_ngspice(w);
    let same = best_ngspice_requested_only(w);
    let p = w.size;
    let mut speed = work_speed_cell(best, rs);
    if best.is_some_and(|b| b.stats.name.starts_with(NG_SENS_PREFIX)) {
        // the ratio against `.sens` is not equal output work: say so right next to it
        let _ = write!(
            speed,
            " (`.sens` computes {})",
            w.sens_vectors.map_or(
                "more than the requested gradients".to_string(),
                |n| format!(
                    "{:.1} sensitivities per requested gradient",
                    n as f64 / p as f64
                )
            )
        );
    }
    if let (Some(b), Some(s)) = (best, same) {
        if b.stats.name != s.stats.name {
            let _ = write!(
                speed,
                "; {} vs {} (the fastest variant delivering only the {p} requested gradients)",
                fmt_x(work_ratio(Some(s), rs).0),
                s.stats.name
            );
        }
    }
    let mut qual: Vec<String> = Vec::new();
    if let Some(b) = best.filter(|b| b.stats.name.starts_with(NG_SENS_PREFIX)) {
        qual.push(format!(
            "`{}` computes and writes {} sensitivity vectors for the {p} requested gradients and its gradient error is {} (scale-normalised), RustSpice's adjoint returns exactly the {p} requested ones",
            b.stats.name,
            w.sens_vectors.map_or("an unknown number of".to_string(), |n| n.to_string()),
            fmt_sci(b.gradient.as_ref().and_then(|g| g.max_over_scale))
        ));
        match same {
            Some(s) => qual.push(format!(
                "fastest variant with the same output ({}) has gradient error {}",
                s.stats.name,
                fmt_sci(s.gradient.as_ref().and_then(|g| g.max_over_scale))
            )),
            None => qual.push(
                "no ngspice variant delivering only the requested gradients completed with a valid accuracy figure".to_string(),
            ),
        }
    }
    let all: Vec<&ContenderStats> = w.contenders.iter().map(|c| &c.stats).collect();
    vec![
        format!(
            "b5 sensitivity ({p} params, vs best ngspice variant: {})",
            best.map_or("n/a", |b| b.stats.name.as_str())
        ),
        fmt_opt_time(med_of(rs)),
        fmt_opt_time(med_of(best)),
        speed,
        "n/a (see b5)".to_string(),
        "n/a (see b5)".to_string(),
        format!(
            "{} / {} (scale-normalised, vs the dense f64 reference, whose own error is {DENSE_REF_OWN_ERROR})",
            // accuracy comes from the correctness run and does not depend on the timing repetitions
            fmt_sci(
                work_contender(w, "rustspice-adjoint")
                    .and_then(|c| c.gradient.as_ref())
                    .and_then(|g| g.max_over_scale)
            ),
            fmt_sci(best.and_then(|b| b.gradient.as_ref()).and_then(|g| g.max_over_scale))
        ),
        "n/a (no ngspice reference run for gradients)".to_string(),
        format!(
            "{} / {}",
            n_cell(work_contender(w, "rustspice-adjoint").map(|c| &c.stats)),
            n_cell(best.map(|c| &c.stats))
        ),
        flags_cell(&w.noisy, &all),
        if qual.is_empty() {
            "-".to_string()
        } else {
            qual.join("; ")
        },
    ]
}

fn batch_headline_row(w: &WorkResult) -> Vec<String> {
    let rs = valid_contender(w, "rustspice-batch");
    let best = best_ngspice(w);
    let best_rs = best_rustspice(w);
    let mut qual: Vec<String> = Vec::new();
    if let (Some(a), Some(b)) = (best_rs, best) {
        qual.push(format!(
            "RustSpice best path `{}` {} vs ngspice best path `{}` {}: {} (>1: RustSpice faster); the batch API row uses {} thread(s), the ngspice best path {}",
            a.stats.name,
            fmt_opt_time(med_of(Some(a))),
            b.stats.name,
            fmt_opt_time(med_of(Some(b))),
            fmt_x(work_ratio(Some(b), Some(a)).0),
            rs.map_or("n/a".to_string(), |c| c.stats.threads.to_string()),
            b.stats.threads
        ));
    }
    let all: Vec<&ContenderStats> = w.contenders.iter().map(|c| &c.stats).collect();
    // accuracy comes from the correctness run and does not depend on the timing repetitions
    let sweep = work_contender(w, "ngspice-dc-sweep-sparse");
    let rs_acc = work_contender(w, "rustspice-batch");
    vec![
        format!(
            "b6 batch ({} corners, RustSpice batch API vs best ngspice variant: {})",
            w.size,
            best.map_or("n/a", |b| b.stats.name.as_str())
        ),
        fmt_opt_time(med_of(rs)),
        fmt_opt_time(med_of(best)),
        work_speed_cell(best, rs),
        "n/a (see b6)".to_string(),
        "n/a (see b6)".to_string(),
        match &w.oracle {
            Some(o) if o.rs.is_some() => format!(
                "{} / {} V (3 probe nodes, all corners; tight reference itself: {})",
                fmt_sci(o.rs),
                fmt_sci(o.ng_default),
                fmt_sci(o.reference)
            ),
            Some(o) => format!("n/a (oracle `{}`: {})", o.spec, o.status),
            None => "n/a (no oracle result)".to_string(),
        },
        match rs_acc.and_then(|c| c.corners.as_ref()) {
            Some(t) => format!(
                "{} / {} V (all nodes and corners)",
                fmt_agree(Some(t.max_abs)),
                fmt_agree(sweep.and_then(|c| c.corners.as_ref()).map(|t| t.max_abs))
            ),
            None => "n/a".to_string(),
        },
        format!(
            "{} / {}",
            n_cell(rs_acc.map(|c| &c.stats)),
            n_cell(best.map(|c| &c.stats))
        ),
        flags_cell(&w.noisy, &all),
        if qual.is_empty() {
            "-".to_string()
        } else {
            qual.join("; ")
        },
    ]
}

fn headline_section(r: &Report) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (bench, label) in [
        ("b1", "b1 DC ladder"),
        ("b2", "b2 RC line tran"),
        ("b3", "b3 diode sweep"),
        ("b4", "b4 CMOS chain tran"),
    ] {
        if let Some(c) = legacy_case(r, bench) {
            rows.push(case_headline_row(r, c, label));
        }
    }
    if let Some(w) = legacy_work(&r.sensitivity) {
        rows.push(sens_headline_row(w));
    }
    if let Some(w) = legacy_work(&r.batch) {
        rows.push(batch_headline_row(w));
    }
    md_table(
        &[
            "Benchmark (legacy size)",
            "RustSpice e2e",
            "ngspice e2e (faster solver)",
            "e2e speedup, median (min) [headline]",
            "analysis-only speedup, median (min) [ngspice analysis / RustSpice analysis, EXCLUDING RustSpice compile]",
            "like-for-like analysis speedup, median (min) [ngspice analysis / RustSpice compile + analysis]",
            "accuracy vs independent oracle: RustSpice / ngspice default",
            "agreement with tight reference: RustSpice / ngspice default",
            "reps n: RustSpice / ngspice",
            "noisy / failed",
            "qualifier",
        ],
        &rows,
    )
}

fn changes_section(r: &Report) -> String {
    let mut s = String::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for p in &r.previously_published {
        let now = match p.id {
            "b5" => legacy_work(&r.sensitivity).map(|w| {
                let rs = valid_contender(w, "rustspice-adjoint");
                let best = best_ngspice(w);
                (
                    fmt_opt_time(med_of(rs)),
                    fmt_opt_time(med_of(best)),
                    fmt_x(work_ratio(best, rs).0),
                )
            }),
            "b6" => legacy_work(&r.batch).map(|w| {
                let rs = valid_contender(w, "rustspice-batch");
                let best = best_ngspice(w);
                (
                    fmt_opt_time(med_of(rs)),
                    fmt_opt_time(med_of(best)),
                    fmt_x(work_ratio(best, rs).0),
                )
            }),
            id => legacy_case(r, id).map(|c| {
                let sp = c.speedups.clone().unwrap_or_default();
                let med = |n: &str| {
                    c.stat(n)
                        .filter(|s| s.completed())
                        .and_then(|s| s.e2e_us.as_ref())
                        .map(|s| s.median)
                };
                (
                    fmt_opt_time(med(RS)),
                    fmt_opt_time(
                        [NG_SPARSE, NG_KLU]
                            .iter()
                            .filter_map(|n| med(n))
                            .reduce(f64::min),
                    ),
                    fmt_x(sp.e2e_median.vs_fastest),
                )
            }),
        };
        let (rs, ng, x) = now.unwrap_or(("n/a".into(), "n/a".into(), "n/a".into()));
        rows.push(vec![
            format!("{} {}", p.id, p.name),
            format!(
                "ngspice {:.2} ms wall / {:.2} ms solver; RustSpice {:.3} ms total / {:.3} ms solver; max dV {}",
                p.ng_wall_ms, p.ng_solver_ms, p.rs_total_ms, p.rs_solver_ms, p.max_dv
            ),
            format!("{:.1}x e2e / {:.1}x solver", p.e2e_speedup, p.solver_speedup),
            rs,
            ng,
            x,
        ]);
    }
    s.push_str(&md_table(
        &[
            "Benchmark",
            "Previously published (old harness, not re-measured)",
            "Previously published speedups",
            "RustSpice e2e now",
            "ngspice e2e now (best variant)",
            "Speedup now (median, e2e)",
        ],
        &rows,
    ));
    s.push_str("\nThe previously published figures come from the old harness on a different host and load; they are listed for provenance only and are NOT comparable with this run's numbers (no ratio between the two is formed).\n");
    s.push('\n');
    let mut bullets: Vec<String> = Vec::new();
    let p = |id: &str| r.previously_published.iter().find(|p| p.id == id);
    for bench in ["b1", "b2", "b3", "b4"] {
        let (Some(c), Some(pb)) = (legacy_case(r, bench), p(bench)) else {
            continue;
        };
        let ng = c.stat(NG_SPARSE).filter(|s| s.completed());
        let rs = c.stat(RS).filter(|s| s.completed());
        let med = |s: Option<&Summary>| s.map(|s| s.median);
        let ng_analysis = med(ng.and_then(|s| s.analysis_us.as_ref()));
        let rs_analysis = med(rs.and_then(|s| s.analysis_us.as_ref()));
        let rs_compile = med(rs.and_then(|s| s.compile_us.as_ref()));
        let rs_both = med(rs.and_then(|s| s.compile_analysis_us.as_ref()));
        let mut line = format!(
            "- **{bench}**: the published ngspice \"solver\" time ({:.2} ms) was the 1 ms-quantised `rusage` elapsed time of a cold process; ngspice's own `Total analysis time` measured in this run is {}. ",
            pb.ng_solver_ms,
            fmt_opt_time(ng_analysis),
        );
        let _ = write!(
            line,
            "The published RustSpice numbers ({:.3} ms total / {:.3} ms solver) timed a warm in-process call; as a subprocess comparable to ngspice it measures {} end-to-end, {} analysis-only (which excludes its {} compile / symbolic-analysis setup) and {} compile + analysis. ",
            pb.rs_total_ms,
            pb.rs_solver_ms,
            fmt_opt_time(rs.and_then(|s| s.e2e_us.as_ref()).map(|s| s.median)),
            fmt_opt_time(rs_analysis),
            fmt_opt_time(rs_compile),
            fmt_opt_time(rs_both),
        );
        if let Some(sp) = &c.speedups {
            let _ = write!(
                line,
                "ngspice was labelled KLU but ran SPARSE; against the faster of the two solvers the end-to-end speedup measured in this run is {} (published: {:.1}x, from a different host and load).",
                fmt_x(sp.e2e_median.vs_fastest),
                pb.e2e_speedup
            );
        }
        bullets.push(line);
        let a = &c.accuracy;
        let acc_line = match (bench, a.status.as_str()) {
            (_, s) if s != "ok" => format!("  - accuracy: {s}"),
            ("b1", _) => {
                // the nodes the old harness compared (v(n/2) and v(n+1)), from the exact recursion of this run's ladder
                let n = c.size;
                let v = ladder_voltages(n, R_SER, R_SH_LEGACY, 5.0);
                let (mid, last) = (v[n / 2 - 1], v[n]);
                format!(
                    "  - accuracy: the published `max |dV| = {}` compared two far-end nodes whose exact voltages are v({}) = {:.2e} V and v({}) = {:.2e} V (the legacy ladder attenuates its 5 V input by {:.2e}x over its length); {} of {} nodes are above the 1 uV floor. Measured now: max |dV| over all nodes {} V (RustSpice) vs {} (ngspice default), against the tight reference; against the independent exact recursion: RustSpice {} V, ngspice default {} V, tight reference {} V.",
                    pb.max_dv,
                    n / 2,
                    mid,
                    n + 1,
                    last,
                    5.0 / last,
                    a.probes_compared,
                    a.probes_total,
                    fmt_sci(a.rs_abs_v),
                    fmt_agree_v(a.ng_abs_v),
                    fmt_sci(a.rs_vs_oracle),
                    fmt_sci(a.ng_vs_oracle),
                    fmt_sci(a.ref_vs_oracle)
                )
            }
            ("b2", _) => format!(
                "  - accuracy: the published `{}` was a hard-coded constant. Measured now over the full waveform at the published 10 ps step: max |dV| {} V ({} of the 3.3 V swing; ngspice default tolerances: {} V).{}",
                pb.max_dv,
                fmt_sci(a.rs_abs_v),
                fmt_pct(a.rs_abs_v.map(|v| v / 3.3)),
                fmt_sci(a.ng_abs_v),
                unequal_accuracy_text(a).map_or(String::new(), |t| format!(" The speedup above is at {t}."))
            ),
            ("b4", _) => format!(
                "  - accuracy: the published `{}` compared only the t = 0 sample of the transient. Measured now over the full waveform: max |dV| {} V (norm. {}; ngspice default: {} V / {}).{}",
                pb.max_dv,
                fmt_sci(a.rs_abs_v),
                fmt_sci(a.rs_err),
                fmt_sci(a.ng_abs_v),
                fmt_sci(a.ng_err),
                unequal_accuracy_text(a).map_or(String::new(), |t| format!(" The speedup above is at {t}."))
            ),
            _ => format!(
                "  - accuracy: published `{}` compared one node; measured now over the full sweep and all nodes: max |dV| {} V (ngspice default: {} V; against the independent oracle: RustSpice {} V, ngspice default {} V, tight reference {} V).",
                pb.max_dv,
                fmt_sci(a.rs_abs_v),
                fmt_sci(a.ng_abs_v),
                fmt_sci(a.rs_vs_oracle),
                fmt_sci(a.ng_vs_oracle),
                fmt_sci(a.ref_vs_oracle)
            ),
        };
        bullets.push(acc_line);
    }
    if let (Some(w), Some(pb)) = (legacy_work(&r.sensitivity), p("b5")) {
        let adj = valid_contender(w, "rustspice-adjoint");
        let line = format!(
            "- **b5**: the published {:.0}x divided {:.2} ms (= 2 x 200 x one cold ngspice run of {:.2} ms, never actually run 400 times) by {:.3} ms (a warm in-process adjoint call). Measured now, e2e median of the RustSpice adjoint process = {}; ratio of each ngspice alternative to it (`.sens` also computes and writes the sensitivities to every other device parameter, the finite-difference variants deliver only the requested gradients): {}.",
            pb.e2e_speedup,
            pb.ng_wall_ms,
            pb.ng_wall_ms / 400.0,
            pb.rs_total_ms,
            fmt_opt_time(med_of(adj)),
            w.contenders
                .iter()
                .filter(|c| c.stats.name.starts_with("ngspice"))
                .map(|c| format!("{} {}", c.stats.name, fmt_x(c.e2e_ratio_vs_rs)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        bullets.push(line);
    }
    if let (Some(w), Some(pb)) = (legacy_work(&r.batch), p("b6")) {
        let line = format!(
            "- **b6**: the published {:.0}x multiplied ONE ngspice run ({:.2} ms: an OP plus a 201-point `.dc` sweep) by 128 and compared it with 128 operating points; ngspice's parallelism was ignored and the corner error was a hard-coded 0. Measured now, e2e median of the RustSpice batch API = {} ({} thread(s)); ratio of each ngspice alternative running the same corners to it: {}.",
            pb.e2e_speedup,
            pb.ng_wall_ms / 128.0,
            fmt_opt_time(med_of(valid_contender(w, "rustspice-batch"))),
            valid_contender(w, "rustspice-batch").map_or("n/a".to_string(), |c| c.stats.threads.to_string()),
            w.contenders
                .iter()
                .filter(|c| c.stats.name.starts_with("ngspice"))
                .map(|c| format!("{} {}", c.stats.name, fmt_x(c.e2e_ratio_vs_rs)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        bullets.push(line);
    }
    s.push_str(&bullets.join("\n"));
    s.push('\n');
    s
}

/// A time cell of one contender: the median when it completed every repetition, else why it has no result.
fn time_cell(
    s: Option<&ContenderStats>,
    pick: impl Fn(&ContenderStats) -> &Option<Summary>,
) -> String {
    match s {
        None => "n/a".to_string(),
        Some(s) if s.status != "ok" => s.status.clone(),
        Some(s) if !s.completed() => format!("n/a ({} of {} repetitions)", s.n, s.reps_planned),
        Some(s) => med(pick(s)),
    }
}

fn timing_section(cases: &[CaseResult], bench_filter: &dyn Fn(&CaseResult) -> bool) -> String {
    let sel: Vec<&CaseResult> = cases.iter().filter(|c| bench_filter(c)).collect();
    let mut t1: Vec<Vec<String>> = Vec::new();
    let mut t2: Vec<Vec<String>> = Vec::new();
    for c in &sel {
        let e = |n: &str| time_cell(c.stat(n), |s| &s.e2e_us);
        let sp = c.speedups.clone().unwrap_or_default();
        let legacy = if c.legacy_size { " (legacy size)" } else { "" };
        t1.push(vec![
            format!("{}{legacy}", c.id),
            e(RS),
            e(NG_SPARSE),
            e(NG_KLU),
            speed_cell(&sp.e2e_median, &sp.e2e_min),
            fmt_x(sp.e2e_net_of_startup),
            if c.noisy.is_empty() {
                "-".to_string()
            } else {
                c.noisy.join(", ")
            },
            format!(
                "{} / {} / {}",
                n_cell(c.stat(RS)),
                n_cell(c.stat(NG_SPARSE)),
                n_cell(c.stat(NG_KLU))
            ),
        ]);
        let analysis = |n: &str| time_cell(c.stat(n), |s| &s.analysis_us);
        let rs = c.stat(RS);
        let ng = c.stat(NG_SPARSE);
        t2.push(vec![
            c.id.clone(),
            format!("{} ({})", analysis(RS), rs_solver(c)),
            time_cell(rs, |s| &s.compile_analysis_us),
            analysis(NG_SPARSE),
            analysis(NG_KLU),
            speed_cell(&sp.analysis_median, &sp.analysis_min),
            speed_cell(&sp.like_for_like_median, &sp.like_for_like_min),
            format!(
                "{} / {} / {}",
                rs.map_or("n/a".into(), |s| med(&s.parse_us)),
                rs.map_or("n/a".into(), |s| med(&s.compile_us)),
                rs.map_or("n/a".into(), |s| med(&s.write_us))
            ),
            format!(
                "{} / {}",
                rs.map_or("n/a".into(), |s| med(&s.overhead_us)),
                ng.map_or("n/a".into(), |s| med(&s.overhead_us))
            ),
            format!(
                "{} / {}",
                fmt_mib(rs.and_then(|s| s.rss_kib.as_ref()).map(|s| s.median)),
                fmt_mib(ng.and_then(|s| s.rss_kib.as_ref()).map(|s| s.median))
            ),
        ]);
    }
    let mut out = String::new();
    out.push_str("End-to-end subprocess wall time (median over interleaved repetitions; both tools write a rawfile to tmpfs). The net-of-floor speedup is `n/a` when the RustSpice net time is below 5x the standard deviation of its start-up floor:\n\n");
    out.push_str(&md_table(
        &[
            "Case",
            "RustSpice",
            "ngspice SPARSE",
            "ngspice KLU",
            "e2e speedup, median (min), vs faster solver",
            "speedup net of start-up floors",
            "noisy",
            "reps n: RustSpice / SPARSE / KLU",
        ],
        &t1,
    ));
    out.push_str("\nAnalysis time and overhead (medians). ngspice's `Total analysis time` includes ngspice's own matrix setup; RustSpice books its symbolic analysis / JIT setup as `compile`, which the analysis-only figures EXCLUDE. The analysis-only speedup (ngspice `Total analysis time` / RustSpice `analysis_us`) is therefore not like-for-like; the like-for-like speedup divides by RustSpice `compile + analysis`:\n\n");
    out.push_str(&md_table(
        &[
            "Case",
            "RustSpice analysis-only (solver)",
            "RustSpice compile + analysis",
            "ngspice SPARSE analysis",
            "ngspice KLU analysis",
            "analysis-only speedup, median (min) [excludes RustSpice compile]",
            "like-for-like speedup, median (min) [ngspice analysis / RustSpice compile + analysis]",
            "RustSpice parse / compile / write",
            "overhead e2e - analysis-only: RustSpice / ngspice",
            "RSS MiB: RustSpice VmHWM / ngspice self-reported",
        ],
        &t2,
    ));
    out
}

/// `RustSpice <= ngspice?`: judged by the oracle where one exists, with a tie band (a 0.4 % difference is not "no").
fn verdict_cell(a: &Accuracy) -> String {
    let Some((rs, ng, basis)) = accuracy_pair(a) else {
        return "n/a".to_string();
    };
    if basis == "tight reference" && ng == 0.0 {
        return "n/a (ngspice default run is identical to the reference)".to_string();
    }
    let verdict = if rs <= ng {
        "yes".to_string()
    } else if rs < UNEQUAL_ACCURACY_FACTOR * ng {
        format!("about equal, within {UNEQUAL_ACCURACY_FACTOR:.0}x")
    } else if ng > 0.0 {
        format!("no, {} larger", fmt_x(Some(rs / ng)))
    } else {
        "no (ngspice default error is 0)".to_string()
    };
    format!("{verdict} ({basis})")
}

fn accuracy_section(cases: &[CaseResult], bench_filter: &dyn Fn(&CaseResult) -> bool) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for c in cases.iter().filter(|c| bench_filter(c)) {
        let a = &c.accuracy;
        rows.push(vec![
            c.id.clone(),
            a.metric.clone(),
            match &a.oracle {
                Some(o) => format!(
                    "{o}: RustSpice {} / ngspice default {} / tight reference itself {}",
                    fmt_sci(a.rs_vs_oracle),
                    fmt_sci(a.ng_vs_oracle),
                    fmt_sci(a.ref_vs_oracle)
                ),
                None => "n/a (no oracle for this circuit)".to_string(),
            },
            if a.status == "ok" {
                fmt_agree(a.rs_err)
            } else {
                a.status.clone()
            },
            fmt_agree(a.ng_err),
            verdict_cell(a),
            fmt_sci(a.rs_abs_v),
            fmt_sci(a.ng_abs_v),
            format!(
                "{} / {} / {}",
                a.probes_compared, a.probes_vacuous, a.probes_total
            ),
            a.ref_status.clone(),
        ]);
    }
    let mut s = md_table(
        &[
            "Case",
            "metric",
            "independent oracle (same metric as the agreement columns)",
            "RustSpice agreement with tight reference",
            "ngspice default agreement with tight reference",
            "RustSpice <= ngspice? (judged by)",
            "RustSpice max |dV| (V) vs reference",
            "ngspice default max |dV| (V) vs reference",
            "probes compared / vacuous / total",
            "reference",
        ],
        &rows,
    );
    s.push_str("\nAgreement with the tight reference is a distance to a reference that has an error of its own (last figure of the oracle column); it bounds the RustSpice agreement from below and is not RustSpice's accuracy. Where an independent oracle exists, the oracle column is the accuracy.\n");
    let tran_rows: Vec<Vec<String>> = cases
        .iter()
        .filter(|c| bench_filter(c) && c.analysis == "tran")
        .map(|c| {
            let a = &c.accuracy;
            let rows_of = |n: Option<usize>| n.map_or("n/a".to_string(), |n| n.to_string());
            let pair = |r: Option<f64>, n: Option<f64>| {
                let ps =
                    |v: Option<f64>| v.map_or("n/a".to_string(), |v| format!("{:.2} ps", v * 1e12));
                format!("{} / {}", ps(r), ps(n))
            };
            vec![
                c.id.clone(),
                pair(a.rs_edge_shift_s, a.ng_edge_shift_s),
                pair(a.rs_edge_shift_at_rows_s, a.ng_edge_shift_at_rows_s),
                pair(a.rs_edge_row_floor_s, a.ng_edge_row_floor_s),
                a.worst_probe.clone().unwrap_or_else(|| "n/a".to_string()),
                a.worst_at
                    .map_or("n/a".to_string(), |t| format!("{:.2} ns", t * 1e9)),
                format!(
                    "{} / {} / {}",
                    rows_of(a.points_rs),
                    rows_of(a.points_ng),
                    rows_of(a.points_ref)
                ),
                a.ref_selfcheck
                    .map_or("n/a (not run for this case)".to_string(), |v| {
                        format!(
                            "{} ({})",
                            fmt_sci(Some(v)),
                            a.ref_selfcheck_label.as_deref().unwrap_or("")
                        )
                    }),
            ]
        })
        .collect();
    if !tran_rows.is_empty() {
        s.push_str("\nTransient timing and reference convergence (50 % crossing shift = largest time shift of a 50 % crossing against the reference; self-check = the reference judged against a still tighter ngspice run, same normalised metric):\n\n");
        s.push_str(&md_table(
            &[
                "Case",
                "max 50 % crossing shift, rawfile rows: RustSpice / ngspice default",
                "same, reference read at each tool's rows: RustSpice / ngspice default",
                "resolution of the rows (reference read at them): RustSpice / ngspice default",
                "RustSpice worst probe",
                "worst at",
                "rawfile rows: RustSpice / ngspice default / reference",
                "reference self-check",
            ],
            &tran_rows,
        ));
        s.push_str("\n`ngspice -b -r` writes its own adaptive internal time points (the row counts above), not a uniform grid. The comparison interpolates RustSpice, ngspice and the reference linearly onto the nominal `k * tstep` grid, so the ngspice error reported here depends on that comparison grid.\n");
        s.push_str("\nThe first crossing column locates every crossing by linear interpolation between the tool's own rows, as a reader of the rawfile would. Rows that are coarser than an edge cannot place its crossing even when every row value is exact: the third column is that limit, measured on the reference itself read only at the tool's rows. The second column reads the reference at the same rows as the tool, so it is the simulator's own timing error. RustSpice writes the nominal `k * tstep` rows; `rustspice run --internal-points` also writes every accepted step in between, as ngspice does, and resolves edges shorter than `tstep`.\n");
    }
    let mut notes: Vec<String> = Vec::new();
    for c in cases.iter().filter(|c| bench_filter(c)) {
        let a = &c.accuracy;
        if let (Some(o), Some(st)) = (&a.oracle, &a.oracle_status) {
            if st != "ok" {
                notes.push(format!("- {}: oracle `{o}` unavailable: {st}", c.id));
            }
        }
        for n in &a.notes {
            notes.push(format!("- {}: {n}", c.id));
        }
    }
    if !notes.is_empty() {
        s.push('\n');
        s.push_str(&notes.join("\n"));
        s.push('\n');
    }
    s
}

fn work_section(ws: &[WorkResult], grad: bool) -> String {
    let mut out = String::new();
    for w in ws {
        let _ = writeln!(
            out,
            "\n#### {}{}\n\n{}. Reference: {} ({}). Repetitions: {}. Load average {} -> {}.\n",
            w.title,
            if w.legacy_size { " (legacy size)" } else { "" },
            w.detail,
            w.reference,
            w.reference_status,
            w.reps,
            w.loadavg_start
                .map_or("n/a".to_string(), |l| format!("{l:.1}")),
            w.loadavg_end
                .map_or("n/a".to_string(), |l| format!("{l:.1}")),
        );
        let mut rows: Vec<Vec<String>> = Vec::new();
        for c in &w.contenders {
            let st = &c.stats;
            let acc = if grad {
                c.gradient.as_ref().map_or(c.accuracy_status.clone(), |g| {
                    format!(
                        "{} (scale-normalised), {} (per-gradient)",
                        fmt_sci(g.max_over_scale),
                        fmt_sci(g.max_rel_significant)
                    )
                })
            } else {
                c.corners.as_ref().map_or(c.accuracy_status.clone(), |t| {
                    format!(
                        "{} V (rel {})",
                        fmt_sci(Some(t.max_abs)),
                        fmt_sci(Some(t.max_rel))
                    )
                })
            };
            rows.push(vec![
                st.name.clone(),
                md_escape(&st.note),
                st.launches.to_string(),
                st.threads.to_string(),
                if st.status == "ok" {
                    med_min(&st.e2e_us)
                } else {
                    st.status.clone()
                },
                med(&st.analysis_us),
                // ngspice reports one analysis time, which already includes its setup
                if st.name.starts_with("rustspice") {
                    med(&st.compile_analysis_us)
                } else {
                    med(&st.analysis_us)
                },
                fmt_x(c.e2e_ratio_vs_rs),
                acc,
                n_cell(Some(st)),
            ]);
        }
        out.push_str(&md_table(
            &[
                "Contender",
                "What it runs",
                "process launches",
                "threads (as reported by the workload)",
                "e2e median (min)",
                "analysis-only median (RustSpice: excludes compile)",
                "like-for-like analysis median (RustSpice: compile + analysis)",
                "e2e time / RustSpice baseline",
                if grad {
                    "gradient error vs dense f64 reference"
                } else {
                    "corner error vs tight ngspice sweep"
                },
                "reps n",
            ],
            &rows,
        ));
        if let Some(o) = &w.oracle {
            let _ = writeln!(
                out,
                "\nIndependent oracle `{}` ({}): RustSpice {}, ngspice default {}, tight reference {} (max |dV| over three probe nodes and all corners).",
                o.spec,
                o.status,
                fmt_sci(o.rs),
                fmt_sci(o.ng_default),
                fmt_sci(o.reference)
            );
        }
        for n in &w.notes {
            let _ = writeln!(out, "\n- {n}");
        }
    }
    out
}

fn series_section(series: &[SeriesResult]) -> String {
    let mut out = String::new();
    for s in series {
        let _ = writeln!(out, "\n#### {}\n", s.title);
        let rows: Vec<Vec<String>> = s
            .points
            .iter()
            .map(|p| {
                vec![
                    format!("{:.1e} s", p.tstep_s),
                    fmt_sci(p.rs_err),
                    fmt_sci(p.ng_err),
                    fmt_opt_time(p.rs_e2e_us),
                    fmt_opt_time(p.ng_e2e_us),
                    fmt_opt_time(p.rs_analysis_us),
                    fmt_opt_time(p.rs_compile_analysis_us),
                    fmt_opt_time(p.ng_analysis_us),
                ]
            })
            .collect();
        out.push_str(&md_table(
            &[
                "tstep",
                "RustSpice norm. error vs reference",
                "ngspice (tmax = tstep, default tolerances) norm. error",
                "RustSpice e2e",
                "ngspice SPARSE e2e",
                "RustSpice analysis-only (excludes compile)",
                "RustSpice compile + analysis",
                "ngspice SPARSE analysis",
            ],
            &rows,
        ));
        for (label, ee) in [
            ("end-to-end", &s.equal_error_e2e),
            (
                "analysis-only, RustSpice without its compile time: not like-for-like",
                &s.equal_error_analysis,
            ),
            (
                "like-for-like analysis, ngspice analysis vs RustSpice compile + analysis",
                &s.equal_error_like_for_like,
            ),
        ] {
            match ee {
                Some(e) if e.status == "ok" => {
                    let _ = writeln!(out, "\nTime at equal error ({label}), log-log interpolated over errors {:.1e} .. {:.1e}:\n", e.range.map_or(0.0, |r| r.0), e.range.map_or(0.0, |r| r.1));
                    let rows: Vec<Vec<String>> = e
                        .levels
                        .iter()
                        .map(|l| {
                            vec![
                                format!("{:.2e}", l.err),
                                fmt_time(l.t_a),
                                fmt_time(l.t_b),
                                fmt_x(Some(l.ratio_b_over_a)),
                            ]
                        })
                        .collect();
                    out.push_str(&md_table(
                        &["error", "RustSpice", "ngspice", "ngspice / RustSpice"],
                        &rows,
                    ));
                }
                Some(e) => {
                    let _ = writeln!(
                        out,
                        "\nTime at equal error ({label}): n/a ({}); RustSpice error range {}, ngspice error range {}.",
                        e.status,
                        e.a_range.map_or("n/a".to_string(), |r| format!("{:.1e} .. {:.1e}", r.0, r.1)),
                        e.b_range.map_or("n/a".to_string(), |r| format!("{:.1e} .. {:.1e}", r.0, r.1)),
                    );
                }
                None => {}
            }
        }
    }
    out
}

fn scaling_section(rows: &[ScalingRow]) -> String {
    let mut by: BTreeMap<(String, String), Vec<&ScalingRow>> = BTreeMap::new();
    for r in rows {
        by.entry((r.family.clone(), r.metric.clone()))
            .or_default()
            .push(r);
    }
    let mut t: Vec<Vec<String>> = Vec::new();
    for ((fam, metric), rs) in by {
        let cell = |name: &str| {
            rs.iter()
                .find(|r| r.contender == name)
                .map_or("n/a".to_string(), |r| match r.fit.exponent {
                    Some(p) => format!("{p:.2} (R2 {:.3})", r.fit.r2.unwrap_or(f64::NAN)),
                    None => format!("n/a ({})", r.fit.status),
                })
        };
        let metric = match metric.as_str() {
            "analysis" => "analysis-only (RustSpice without compile)".to_string(),
            "like_for_like" => "like-for-like analysis (RustSpice compile + analysis)".to_string(),
            m => m.to_string(),
        };
        t.push(vec![fam, metric, cell(RS), cell(NG_SPARSE), cell(NG_KLU)]);
    }
    md_table(
        &[
            "family",
            "metric",
            "RustSpice exponent",
            "ngspice SPARSE exponent",
            "ngspice KLU exponent",
        ],
        &t,
    )
}

fn floors_section(f: &Floors) -> String {
    md_table(
        &[
            "process",
            "start-up floor (trivial .op deck, median)",
            "min",
            "reps",
        ],
        &[
            vec![
                "rustspice run".to_string(),
                med(&f.rustspice_us),
                f.rustspice_us
                    .as_ref()
                    .map_or("n/a".into(), |s| fmt_time(s.min)),
                f.reps.to_string(),
            ],
            vec![
                "ngspice -b (SPARSE)".to_string(),
                med(&f.ngspice_sparse_us),
                f.ngspice_sparse_us
                    .as_ref()
                    .map_or("n/a".into(), |s| fmt_time(s.min)),
                f.reps.to_string(),
            ],
            vec![
                "ngspice -b (KLU)".to_string(),
                med(&f.ngspice_klu_us),
                f.ngspice_klu_us
                    .as_ref()
                    .map_or("n/a".into(), |s| fmt_time(s.min)),
                f.reps.to_string(),
            ],
        ],
    )
}

/// The Markdown report.
pub fn render_markdown(r: &Report) -> String {
    let mut s = String::with_capacity(32 * 1024);
    let _ = writeln!(
        s,
        "# Benchmarks: RustSpice vs. {}\n",
        r.ngspice
            .version
            .as_deref()
            .unwrap_or("ngspice (not installed)")
    );
    let _ = writeln!(
        s,
        "> Generated by `rustspice bench-ngspice` (RustSpice {}{}, {} build) in {:.0} s. Every number below was measured in this run unless it is explicitly labelled *previously published*. **Absolute numbers are valid only for the host and load recorded here; trust ratios within this run, not across runs.**\n",
        r.rustspice.version,
        git_label(r.rustspice.git.as_deref(), r.rustspice.git_dirty),
        r.rustspice.build_profile,
        r.elapsed_s,
    );
    if r.rustspice.git_dirty == Some(true) {
        let _ = writeln!(
            s,
            "> **WARNING: the RustSpice source tree this binary was built from has uncommitted changes; the commit hash above does not identify the code that was measured.**\n"
        );
    }
    if r.rustspice.build_profile != "release" {
        let _ = writeln!(
            s,
            "> **WARNING: debug build. Timings are not representative.**\n"
        );
    }
    if !r.ngspice.installed {
        let _ = writeln!(s, "> **ngspice is not installed: every comparison below is `unverified` (n/a), only RustSpice-side numbers are shown.**\n");
    }
    let _ = writeln!(s, "## Scope of every number\n");
    let _ = writeln!(
        s,
        "- **Host**: {} ({}). Load average {} at start, {} at end; sequential children {}. Scratch files on {}. Parent peak RSS {} MiB.",
        r.host.cpu,
        threads_label(r.host.hardware_threads, r.host.threads),
        r.host.loadavg_start.map_or("n/a".to_string(), |l| format!("{l:.1}")),
        r.host.loadavg_end.map_or("n/a".to_string(), |l| format!("{l:.1}")),
        r.host.pin_cpu.map_or("not pinned to a CPU".to_string(), |c| format!("pinned to CPU {c} (parallel workloads never pinned)")),
        if r.host.tmp_on_tmpfs { "tmpfs (/dev/shm)" } else { "the system temp dir (NOT tmpfs)" },
        fmt_mib(r.parent_peak_rss_kib.map(|k| k as f64)),
    );
    let _ = writeln!(
        s,
        "- **Simulators**: {} timed with BOTH of its direct solvers: SPARSE 1.3 (its default; the old report mislabelled it KLU) and KLU (`.options klu`); the headline compares against the faster of the two per metric. RustSpice solver kind is shown per case.",
        r.ngspice.version.as_deref().unwrap_or("ngspice"),
    );
    let _ = writeln!(
        s,
        "- **Temperature**: {} C (300.15 K) in both tools (RustSpice has a fixed 300.15 K).",
        r.scope.temperature_c
    );
    let _ = writeln!(
        s,
        "- **Tolerances**: ngspice as-is: {}. RustSpice: {}. Reference runs: op/dc {}; tran {}.",
        r.scope.ngspice_asis_tolerances,
        r.scope.rustspice_tolerances,
        r.scope.ngspice_reference_op_dc,
        r.scope.ngspice_reference_tran
    );
    let _ = writeln!(s, "- **Step policy**: {}", r.scope.step_policy);
    let _ = writeln!(
        s,
        "- **Timing protocol**: both contenders are subprocesses writing a rawfile (`rustspice run --raw --stats`, `ngspice -b -r`). One warm-up run (also the correctness run), then {} interleaved repetitions in rotating order ({}). A contender whose OWN warm-up takes over 1 s / 5 s is repeated at most 5 / 3 times, and heavy multi-launch workloads (each repetition already averages many launches) at most 3; a cap applies only to that contender, never to the others. Every table shows the repetitions n actually completed, and a contender that failed in any repetition never enters a headline ratio. Statistics: min, median, stddev; a series is `noisy` when stddev / median > 0.2. End-to-end = wall time of the child including start-up (the headline). Analysis-only = the tool's own analysis time: ngspice `Total analysis time` (which includes ngspice's matrix setup) and RustSpice `analysis_us`, which EXCLUDES RustSpice's symbolic analysis / JIT setup booked as `compile_us`; the analysis-only ratio is therefore not like-for-like, and the like-for-like ratio divides by RustSpice `compile_us + analysis_us`. Overhead = e2e - analysis-only. Start-up floors are measured separately below.",
        r.scope.reps_requested,
        if r.scope.timing_enabled { "timing enabled" } else { "TIMING DISABLED (--no-timing)" },
    );
    let _ = writeln!(
        s,
        "- **Accuracy protocol**: full waveform / full sweep / all nodes against a tightly converged ngspice reference through the engine's `compare` machinery (probes whose reference never leaves the floor are `vacuous` and excluded; the transient swing floor is {} V). ngspice's own default-tolerance error against the same reference is shown next to RustSpice's, but that distance is bounded below by the reference's own error; where an independent oracle exists (exact ladder recursion, dense Newton, linear-circuit transient solution; `n/a` otherwise) it judges RustSpice, ngspice default and the reference itself, and is the accuracy figure. Never 0 for an unmeasured quantity.\n",
        r.scope.tran_floor_v,
    );

    let _ = writeln!(s, "## Head-to-head at the legacy sizes\n");
    s.push_str(&headline_section(r));
    let _ = writeln!(s, "\nSpeedup = ngspice time / RustSpice time (>1: RustSpice faster), medians (minima in parentheses), against the faster ngspice solver; the end-to-end column is the headline. Accuracy: the oracle column is max |dV| against an independent oracle (an exact or high-precision computation that uses neither simulator) where one exists (volts for op/dc, max |dV| / swing for transients), with the tight ngspice reference's own error in brackets; the agreement column is only the distance to that reference, which has an error of its own and bounds RustSpice's agreement figure from below. A speedup is flagged `unequal accuracy` when RustSpice's error is at least {UNEQUAL_ACCURACY_FACTOR:.0}x ngspice's. `n/a` = not measured or not valid; a contender that failed a repetition or has no valid accuracy figure never enters a headline ratio.\n");

    let _ = writeln!(
        s,
        "## What changed versus the previously published numbers\n"
    );
    s.push_str(&changes_section(r));

    let _ = writeln!(s, "\n## b1: DC operating point of a resistor ladder\n");
    let b1 = |c: &CaseResult| c.bench == "b1";
    s.push_str(&timing_section(&r.cases, &b1));
    let _ = writeln!(s, "\nAccuracy (`b1_ladder_*` is the legacy circuit; `b1n_ladder_flat_*` scales the shunt so that every node carries O(1) volts):\n");
    s.push_str(&accuracy_section(&r.cases, &b1));
    let _ = writeln!(s, "\n{LADDER_ORACLE_NOTE}");

    let _ = writeln!(s, "\n## b2: RC transmission line, transient\n");
    let b2 = |c: &CaseResult| c.bench == "b2";
    s.push_str(&timing_section(&r.cases, &b2));
    let _ = writeln!(s, "\nAccuracy at the published 10 ps step (the reference is converged with a 0.5 ps maximum step):\n");
    s.push_str(&accuracy_section(&r.cases, &b2));
    if !r.series.is_empty() {
        let _ = writeln!(s, "\n### b2 step ladder: time versus accuracy\n");
        s.push_str(&series_section(&r.series));
    }

    let _ = writeln!(s, "\n## b3: nonlinear diode cascade, DC sweep\n");
    let b3 = |c: &CaseResult| c.bench == "b3";
    s.push_str(&timing_section(&r.cases, &b3));
    let _ = writeln!(s, "\nAccuracy:\n");
    s.push_str(&accuracy_section(&r.cases, &b3));

    let _ = writeln!(s, "\n## b4: CMOS inverter chain, transient\n");
    let b4 = |c: &CaseResult| c.bench == "b4";
    s.push_str(&timing_section(&r.cases, &b4));
    let _ = writeln!(
        s,
        "\nAccuracy (full waveform; reference converged with tmax = tstep / 20):\n"
    );
    s.push_str(&accuracy_section(&r.cases, &b4));

    let _ = writeln!(s, "\n## b5: sensitivity of one output to every resistor\n");
    let _ = writeln!(
        s,
        "RustSpice computes all gradients with one forward and one adjoint solve. The ngspice alternatives: `.sens` (one process; it computes and writes the sensitivities to every device parameter, so it does MORE than the requested gradients, and its own gradient error is in the error column), central finite differences in ONE process (`.control` alter loop) and separate launches (2P central / P+1 forward), which deliver exactly the requested gradients. The RustSpice finite-difference loop is shown for reference. Gradient error is max |g - g_ref| divided by max |g_ref| (scale-normalised) and the maximum relative error over gradients above 1e-3 of the largest. The RustSpice rows are the same library calls as `rustspice adjoint`, run in a child process that writes every gradient to a JSON file (so process start-up, parse, compile and output are included, like the ngspice rows). The old published number divided 400 cold ngspice launches that were never run (one launch multiplied by 400) by a warm in-process adjoint call; the launch variants below are what that workload really costs."
    );
    s.push_str(&work_section(&r.sensitivity, true));

    let _ = writeln!(s, "\n## b6: batch of supply corners\n");
    let _ = writeln!(
        s,
        "The same corner values on both sides. RustSpice: rayon batch (all threads and one thread), a sequential in-process loop, and a `.dc` sweep. ngspice: sequential launches, concurrent launches, ONE process with a `.control` loop, and ONE `.dc` sweep (the idiomatic equivalent for supply corners). The RustSpice batch / loop rows run as child processes (same process-level accounting as ngspice); the thread count column is the thread count each workload reports for itself (the rayon pool size, which honours `RAYON_NUM_THREADS`, for the RustSpice batch API; 1 for the sequential RustSpice workers and the single-process ngspice runs). The reference is a tightly converged ngspice `.dc` sweep over the same values; the independent oracle (dense f64 Newton) judges RustSpice, ngspice at default tolerances and the reference itself."
    );
    s.push_str(&work_section(&r.batch, false));

    let _ = writeln!(s, "\n## Scaling over the size ladders\n");
    s.push_str(&scaling_section(&r.scaling));
    let _ = writeln!(s, "\nLog-log least-squares exponents of the median time against the size over the three ladder points (indicative only).\n");

    let _ = writeln!(s, "## Process start-up floors\n");
    s.push_str(&floors_section(&r.floors));
    let _ = writeln!(s, "\n## Reproduce\n");
    let _ = writeln!(s, "```sh\ncargo build --release -p rustspice-cli\n./target/release/rustspice bench-ngspice --output docs/benchmark_report.md --json bench.json\n# options: --reps N (default 7)  --pin-cpu N  --no-timing  --only b1,b5  --ngspice <path>\n```\n");
    let _ = writeln!(s, "## Caveats\n");
    let _ = writeln!(s, "- The host is shared and noisy; `noisy` marks series whose spread exceeds 20 % of the median. Re-run on a quiet machine before quoting absolute times.");
    let _ = writeln!(s, "- Hybrid or frequency-scaling CPUs (performance and efficiency cores, turbo) make short processes bimodal: the same ngspice launch can take 30 ms or 60 ms depending on the core it lands on. Series are interleaved and repeated so both sides see the same mix, but the medians still carry that spread; `--pin-cpu N` pins the sequential children to one core for steadier numbers (parallel workloads are never pinned).");
    let _ = writeln!(s, "- The like-for-like analysis ratio pairs ngspice `Total analysis time` with RustSpice `compile_us + analysis_us`: the closest comparable pair the two tools report, not an exact phase match (their phase boundaries differ; ngspice's figure includes its matrix setup, RustSpice's compile its symbolic analysis / JIT). The end-to-end ratio is the only one that needs no assumption about phase boundaries.");
    let _ = writeln!(s, "- Both transients are adaptive, but with different error controls and output points (RustSpice writes the nominal `k * tstep` rows, ngspice its own internal time points). Their errors at the same nominal step are therefore not the same quantity, which is why the accuracy columns are measured against a converged reference and the b2 step ladder shows time versus accuracy.");
    let _ = writeln!(s, "- The b5 / b6 ngspice multi-launch variants write their decks before timing (deck generation is not timed); the process launches themselves are.");
    s
}

// ---------------------------------------------------------------------------------------------------------
// b2 step ladder (time versus accuracy)
// ---------------------------------------------------------------------------------------------------------

/// Time steps of the b2 work-precision series (the first is the published step).
pub const SERIES_STEPS: [&str; 4] = ["10p", "5p", "2p", "1p"];
/// Maximum step of the shared series reference (seconds).
const SERIES_REF_TMAX: f64 = 0.5e-12;

/// Equal-error tables (end-to-end, analysis-only, like-for-like analysis) from the per-step points; `None` when fewer
/// than two usable points exist.
pub fn series_equal_error(
    points: &[SeriesPoint],
) -> (Option<EqualError>, Option<EqualError>, Option<EqualError>) {
    let curve = |err: fn(&SeriesPoint) -> Option<f64>,
                 t: fn(&SeriesPoint) -> Option<f64>|
     -> Vec<(f64, f64)> {
        points
            .iter()
            .filter_map(|p| Some((err(p)?, t(p)?)))
            .collect()
    };
    let eq = |a: Vec<(f64, f64)>, b: Vec<(f64, f64)>| {
        (a.len() >= 2 && b.len() >= 2).then(|| time_at_equal_error(&a, &b, 5))
    };
    let rs_e2e = curve(|p| p.rs_err, |p| p.rs_e2e_us);
    let ng_e2e = curve(|p| p.ng_err, |p| p.ng_e2e_us);
    let rs_ana = curve(|p| p.rs_err, |p| p.rs_analysis_us);
    let ng_ana = curve(|p| p.ng_err, |p| p.ng_analysis_us);
    let rs_lfl = curve(|p| p.rs_err, |p| p.rs_compile_analysis_us);
    (
        eq(rs_e2e, ng_e2e),
        eq(rs_ana, ng_ana.clone()),
        eq(rs_lfl, ng_ana),
    )
}

/// The b2 step ladder on a `stages`-stage RC line: one shared converged reference, one case per time step.
pub fn run_series(ctx: &Ctx, stages: usize) -> (Vec<CaseResult>, SeriesResult) {
    let mut cases = Vec::new();
    let mut shared: Option<RefRun> = None;
    for (k, ts) in SERIES_STEPS.iter().enumerate() {
        let deck = rc_line_deck(stages, ts);
        let spec = CaseSpec {
            id: format!("b2s_rcline_{stages}_ts{ts}"),
            bench: "b2s",
            circuit: format!("{stages}-stage RC line, .tran {ts} 5n"),
            detail: format!("step ladder member {}/{}", k + 1, SERIES_STEPS.len()),
            size: stages,
            legacy: false,
            kind: PlotKind::Tran,
            deck,
            floor: TRAN_FLOOR_V,
            oracle: None,
            ladder: None,
            ref_tmax: Some(SERIES_REF_TMAX),
            ref_check: false,
        };
        if shared.is_none() && ctx.ng_ok {
            let dir = ctx.tmp.path().join("b2s_ref");
            let _ = fs::create_dir_all(&dir);
            shared = Some(make_reference(
                ctx,
                &spec.deck,
                PlotKind::Tran,
                Some(SERIES_REF_TMAX),
                &dir,
            ));
        }
        cases.push(run_case(ctx, &spec, shared.as_ref()));
    }
    let points: Vec<SeriesPoint> = cases
        .iter()
        .zip(SERIES_STEPS.iter())
        .map(|(c, ts)| {
            // medians of contenders that completed every repetition only
            let med = |n: &str, pick: fn(&ContenderStats) -> &Option<Summary>| {
                c.stat(n)
                    .filter(|s| s.completed())
                    .and_then(|s| pick(s).as_ref())
                    .map(|s| s.median)
            };
            SeriesPoint {
                tstep_s: meta::parse_spice_number(ts).unwrap_or(f64::NAN),
                rs_err: c.accuracy.rs_err,
                ng_err: c.accuracy.ng_err,
                rs_e2e_us: med(RS, |s| &s.e2e_us),
                ng_e2e_us: med(NG_SPARSE, |s| &s.e2e_us),
                rs_analysis_us: med(RS, |s| &s.analysis_us),
                ng_analysis_us: med(NG_SPARSE, |s| &s.analysis_us),
                rs_compile_analysis_us: med(RS, |s| &s.compile_analysis_us),
            }
        })
        .collect();
    let (e2e, analysis, like_for_like) = series_equal_error(&points);
    let res = SeriesResult {
        id: format!("b2s_rcline_{stages}"),
        title: format!(
            "{stages}-stage RC line, tstep 10p..1p (reference: reltol 1e-6, tmax {:e} s)",
            SERIES_REF_TMAX
        ),
        points,
        equal_error_e2e: e2e,
        equal_error_analysis: analysis,
        equal_error_like_for_like: like_for_like,
    };
    (cases, res)
}

// ---------------------------------------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------------------------------------

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown cpu".to_string())
}

/// Output of `git -C <dir> <args>` (stdout, trimmed) when git succeeds.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `(short HEAD, dirty)` of the git tree that contains `dir`: the source tree this binary was built from, NOT the
/// current directory of whoever launches the benchmark. `(None, None)` when `dir` is not inside a git checkout.
pub fn git_state(dir: &Path) -> (Option<String>, Option<bool>) {
    let head = git_in(dir, &["rev-parse", "--short", "HEAD"]).filter(|s| !s.is_empty());
    if head.is_none() {
        return (None, None);
    }
    // any modified, staged or untracked file counts: the hash alone would not identify the measured code
    let dirty = git_in(dir, &["status", "--porcelain"]).map(|s| !s.is_empty());
    (head, dirty)
}

/// ` @ 7bb6d36` / ` @ 7bb6d36-dirty` / empty.
pub fn git_label(head: Option<&str>, dirty: Option<bool>) -> String {
    match (head, dirty) {
        (Some(h), Some(true)) => format!(" @ {h}-dirty"),
        (Some(h), Some(false)) => format!(" @ {h}"),
        (Some(h), None) => format!(" @ {h} (dirty state unknown)"),
        (None, _) => String::new(),
    }
}

/// Logical CPUs of the machine: the `processor` entries of a `/proc/cpuinfo` text.
pub fn count_processors(cpuinfo: &str) -> Option<usize> {
    let n = cpuinfo
        .lines()
        .filter(|l| {
            l.split_once(':')
                .is_some_and(|(k, _)| k.trim() == "processor")
        })
        .count();
    (n > 0).then_some(n)
}

fn hardware_threads() -> Option<usize> {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| count_processors(&s))
}

/// `32 hardware threads, 16 available to this run (affinity / cgroup limited)`.
pub fn threads_label(hardware: Option<usize>, available: usize) -> String {
    match hardware {
        Some(h) if h == available => format!("{h} hardware threads"),
        Some(h) => format!(
            "{h} hardware threads, {available} available to this run (affinity / cgroup limited)"
        ),
        None => format!("hardware thread count unknown, {available} available to this run"),
    }
}

/// Peak resident set (`VmHWM`) of this process in KiB.
fn peak_rss_kib() -> Option<u64> {
    let s = fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn write_out(path: &Path, text: &str) -> bool {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(path, text) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("error: cannot write {}: {e}", path.display());
            false
        }
    }
}

fn progress_line(c: &CaseResult) -> String {
    let t = |n: &str| {
        c.stat(n)
            .and_then(|s| s.e2e_us.as_ref())
            .map_or("n/a".to_string(), |s| fmt_time(s.median))
    };
    let oracle = if c.accuracy.oracle_status.as_deref() == Some("ok") {
        format!(
            " | oracle rs {} ng {}",
            fmt_sci(c.accuracy.rs_vs_oracle),
            fmt_sci(c.accuracy.ng_vs_oracle)
        )
    } else {
        String::new()
    };
    format!(
        "{:<28} rs {:>10} | ng-sparse {:>10} | ng-klu {:>10} | e2e speedup {:>8} | agreement with reference rs {} ng {}{oracle}",
        c.id,
        t(RS),
        t(NG_SPARSE),
        t(NG_KLU),
        fmt_x(c.speedups.as_ref().and_then(|s| s.e2e_median.vs_fastest)),
        fmt_agree(c.accuracy.rs_err),
        fmt_agree(c.accuracy.ng_err),
    )
}

/// `rustspice bench-ngspice`. Returns the process exit code: 0 ok, 1 a RustSpice run failed, 2 environment problem
/// (ngspice missing: the report is still written with every comparison marked unverified).
pub fn run(args: LegacyArgs) -> i32 {
    if let Some(job) = &args.worker {
        return worker_main(job);
    }
    let started = Instant::now();
    let debug_build = cfg!(debug_assertions);
    if debug_build {
        eprintln!("warning: debug build; timings are not representative (build with --release)");
    }
    let known = ["b1", "b2", "b3", "b4", "b5", "b6"];
    for o in &args.only {
        if !known.iter().any(|k| k.eq_ignore_ascii_case(o)) {
            eprintln!("error: --only takes b1,b2,b3,b4,b5,b6 (got `{o}`)");
            return 2;
        }
    }
    let want =
        |b: &str| args.only.is_empty() || args.only.iter().any(|o| o.eq_ignore_ascii_case(b));
    if args.reps < 5 && !args.no_timing {
        eprintln!(
            "warning: --reps {} < 5; timing statistics are weak",
            args.reps
        );
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: cannot locate the rustspice executable: {e}");
            return 2;
        }
    };
    // SIGTERM / SIGINT / SIGHUP kill the children and remove the registered scratch directory (ER-legacy-08)
    rustsim::install_termination_cleanup();
    oracle::set_case_timeout(Duration::from_secs(args.case_timeout));
    let tmp = match TmpDir::new("legacy") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot create a scratch directory: {e}");
            return 2;
        }
    };
    let ng_version = ngspice::version(&args.ngspice);
    let ng_ok = ng_version.is_some();
    if !ng_ok {
        eprintln!(
            "warning: ngspice (`{}`) is not available; every comparison is unverified",
            args.ngspice
        );
    }
    let lim = Limits {
        timeout: Duration::from_secs(args.case_timeout),
        max_rss_kib: Some(CHILD_RSS_CAP_KIB),
        pin_cpu: args.pin_cpu,
    };
    let lim_free = Limits {
        pin_cpu: None,
        ..lim.clone()
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut ctx = Ctx {
        exe,
        ng: args.ngspice.clone(),
        ng_ok,
        ng_version: ng_version.clone(),
        tmp,
        reps: args.reps.max(3),
        timing: !args.no_timing,
        lim,
        lim_free,
        threads,
        floors: Floors {
            status: "skipped (--no-timing)".to_string(),
            ..Default::default()
        },
    };
    let load_start = loadavg();
    println!(
        "bench-ngspice: {} | reps {} | scratch {}",
        ng_version.as_deref().unwrap_or("ngspice NOT installed"),
        ctx.reps,
        ctx.tmp.path().display()
    );
    if ctx.timing {
        ctx.floors = measure_floors(&ctx);
        println!(
            "start-up floors: rustspice {} | ngspice sparse {} | ngspice klu {}",
            med(&ctx.floors.rustspice_us),
            med(&ctx.floors.ngspice_sparse_us),
            med(&ctx.floors.ngspice_klu_us)
        );
    }
    let mut cases: Vec<CaseResult> = Vec::new();
    for spec in case_specs(&args.only) {
        let c = run_case(&ctx, &spec, None);
        println!("{}", progress_line(&c));
        cases.push(c);
    }
    let mut series = Vec::new();
    if want("b2") {
        let (cs, s) = run_series(&ctx, 500);
        for c in &cs {
            println!("{}", progress_line(c));
        }
        cases.extend(cs);
        series.push(s);
    }
    let mut sensitivity = Vec::new();
    if want("b5") {
        for stages in [25usize, 50, 100] {
            let w = run_sensitivity(&ctx, stages, stages == 100);
            println!("{}: {} contenders", w.id, w.contenders.len());
            sensitivity.push(w);
        }
    }
    let mut batch = Vec::new();
    if want("b6") {
        for corners in [32usize, 64, 128] {
            let w = run_batch(&ctx, corners, corners == 128);
            println!("{}: {} contenders", w.id, w.contenders.len());
            batch.push(w);
        }
    }
    let load_end = loadavg();
    let scaling = scaling_rows(&cases);
    let solvers_timed = if ng_ok {
        vec!["sparse".to_string(), "klu".to_string()]
    } else {
        Vec::new()
    };
    let (git_head, git_dirty) = git_state(Path::new(env!("CARGO_MANIFEST_DIR")));
    let report = Report {
        schema: SCHEMA,
        tool: "rustspice bench-ngspice".to_string(),
        elapsed_s: started.elapsed().as_secs_f64(),
        parent_peak_rss_kib: peak_rss_kib(),
        host: HostInfo {
            cpu: cpu_model(),
            threads,
            hardware_threads: hardware_threads(),
            loadavg_start: load_start,
            loadavg_end: load_end,
            pin_cpu: args.pin_cpu,
            tmp_on_tmpfs: ctx.tmp.on_tmpfs(),
        },
        ngspice: NgInfo {
            version: ng_version,
            path: args.ngspice.clone(),
            installed: ng_ok,
            solvers_timed,
        },
        rustspice: RsInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git: git_head,
            git_dirty,
            source_tree: env!("CARGO_MANIFEST_DIR").to_string(),
            build_profile: if debug_build { "debug" } else { "release" }.to_string(),
        },
        scope: ScopeInfo {
            temperature_c: 27.0,
            ngspice_asis_tolerances: "defaults (reltol 1e-3, vntol 1e-6 V, abstol 1e-12 A), SPARSE and KLU".to_string(),
            ngspice_reference_op_dc: format!("`.options {TIGHT_OPTIONS}` (SPARSE)"),
            ngspice_reference_tran: format!("`.options {TRAN_REF_OPTIONS}` and tmax = tstep / 20 (SPARSE)"),
            rustspice_tolerances: "fixed internally (`.options reltol/vntol/abstol` warn and are ignored): Newton reltol 1e-6, vntol 1e-10 V, abstol 1e-12 A; transient LTE reltol 1e-3, vntol 1e-6 V, abstol 1e-12 A, trtol 7".to_string(),
            step_policy: STEP_POLICY.to_string(),
            reps_requested: ctx.reps,
            timing_enabled: ctx.timing,
            tran_floor_v: TRAN_FLOOR_V,
        },
        floors: ctx.floors.clone(),
        cases,
        series,
        sensitivity,
        batch,
        scaling,
        previously_published: PUBLISHED.to_vec(),
    };
    let md = render_markdown(&report);
    let mut ok = write_out(&args.output, &md);
    if let Some(j) = &args.json {
        match serde_json::to_string_pretty(&report) {
            Ok(t) => ok &= write_out(j, &t),
            Err(e) => {
                eprintln!("error: cannot serialise the report: {e}");
                ok = false;
            }
        }
    }
    println!(
        "report written to {} ({:.0} s, parent peak RSS {} MiB)",
        args.output.display(),
        report.elapsed_s,
        fmt_mib(report.parent_peak_rss_kib.map(|k| k as f64))
    );
    let rs_failed = report.cases.iter().any(|c| !c.rustspice_ok)
        || report.sensitivity.iter().chain(&report.batch).any(|w| {
            w.contenders
                .iter()
                .any(|c| c.stats.name.starts_with("rustspice") && c.stats.status != "ok")
        });
    if !ok || !ng_ok || debug_build {
        2
    } else if rs_failed {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::rawfile::Var;
    use crate::bench::stats::EqualErrorLevel;

    fn rel(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs().max(1e-300)
    }

    #[test]
    fn numbers_round_trip_as_spice_text() {
        for x in [10.0, 1e4, 6.25e7, 1e-5, 0.03, 1.5 + 0.03 * 7.0, 5.0] {
            assert_eq!(num(x).parse::<f64>().unwrap(), x, "{}", num(x));
            assert_eq!(meta::parse_spice_number(&num(x)), Some(x), "{}", num(x));
        }
        assert_eq!(num(10.0), "10.0");
    }

    #[test]
    fn formatting_helpers() {
        assert_eq!(fmt_time(0.5), "0.50 µs");
        assert_eq!(fmt_time(412.4), "412 µs");
        assert_eq!(fmt_time(3210.0), "3.21 ms");
        assert_eq!(fmt_time(250_000.0), "250 ms");
        assert_eq!(fmt_time(1.52e6), "1.52 s");
        assert_eq!(fmt_time(f64::NAN), "n/a");
        assert_eq!(fmt_x(Some(5.3)), "5.30x");
        assert_eq!(fmt_x(Some(53.0)), "53.0x");
        assert_eq!(fmt_x(Some(6285.0)), "6285x");
        assert_eq!(fmt_x(None), "n/a");
        assert_eq!(fmt_x(Some(f64::INFINITY)), "n/a");
        assert_eq!(fmt_sci(Some(2.16e-9)), "2.16e-9");
        assert_eq!(fmt_sci(Some(0.0)), "0 (identical)");
        assert_eq!(fmt_sci(None), "n/a");
        assert_eq!(fmt_sci(Some(f64::NAN)), "n/a");
        assert_eq!(fmt_pct(Some(0.0962)), "9.62 %");
        assert_eq!(fmt_pct(None), "n/a");
        assert_eq!(fmt_count(5002), "5,002");
        assert_eq!(fmt_count(10_000), "10,000");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(md_escape("a|b\nc"), "a\\|b c");
    }

    #[test]
    fn decks_are_wellformed_and_free_of_control_blocks() {
        let d = ladder_deck(3, 1e4);
        assert_eq!(
            d,
            "Large Resistor Ladder Power Grid Benchmark\nV1 1 0 DC 5.0\nRser1 1 2 10.0\nRsh1 2 0 10000.0\nRser2 2 3 10.0\nRsh2 3 0 10000.0\nRser3 3 4 10.0\nRsh3 4 0 10000.0\n.op\n.end\n"
        );
        for deck in [
            ladder_deck(50, 1e4),
            rc_line_deck(20, "10p"),
            diode_cascade_deck(8),
            cmos_chain_deck(5),
            floor_deck(),
        ] {
            let low = deck.to_ascii_lowercase();
            assert!(!low.contains(".control") && !low.contains(".endc"));
            assert!(low.trim_end().ends_with(".end"));
            assert!(meta::find_tran(&deck).is_some() || low.contains(".op") || low.contains(".dc"));
        }
        let rc = rc_line_deck(4, "2p");
        assert!(rc.contains(".tran 2p 5n\n"));
        assert_eq!(rc.lines().filter(|l| l.starts_with('R')).count(), 4);
        assert_eq!(rc.lines().filter(|l| l.starts_with('C')).count(), 4);
        let dd = diode_cascade_deck(4);
        assert_eq!(dd.lines().filter(|l| l.starts_with('D')).count(), 4);
        assert_eq!(
            dd.lines().filter(|l| l.starts_with('R')).count(),
            8,
            "R and Rlink"
        );
        assert!(dd.contains(".dc V1 0.0 5.0 0.025"));
        let c = cmos_chain_deck(5);
        assert_eq!(c.lines().filter(|l| l.starts_with("MN")).count(), 5);
        assert_eq!(c.lines().filter(|l| l.starts_with("MP")).count(), 5);
        assert!(c.contains("MP2 n2 n1 vdd vdd PMOD W=20u L=1u"));
        assert!(c.contains(".tran 5p 2n"));
    }

    #[test]
    fn ladder_override_replaces_exactly_one_resistor() {
        let base = ladder_deck(3, 1e4);
        let d = ladder_deck_with(3, 1e4, ".op", Some((3, 12345.5)));
        assert_ne!(base, d);
        assert!(d.contains("Rsh2 3 0 12345.5\n"));
        let changed = base.lines().zip(d.lines()).filter(|(a, b)| a != b).count();
        assert_eq!(changed, 1);
    }

    #[test]
    fn shunt_scaling_makes_the_ladder_non_vacuous() {
        let n = 2000;
        let flat = ladder_voltages(n, R_SER, shunt_for_depth(n, 2.0), 5.0);
        assert!(
            flat.iter().all(|v| *v > 0.5),
            "min {}",
            flat.last().unwrap()
        );
        let legacy = ladder_voltages(n, R_SER, R_SH_LEGACY, 5.0);
        assert!(legacy.iter().filter(|v| **v < 1e-6).count() > n / 2);
    }

    #[test]
    fn ladder_recursion_matches_closed_forms() {
        // one stage: v2 = vin * rsh / (rser + rsh)
        let v = ladder_voltages(1, 10.0, 90.0, 5.0);
        assert!((v[1] - 5.0 * 90.0 / 100.0).abs() < 1e-15);
        // two stages by hand: node 2 sees rsh in parallel with (rser + rsh)
        let (rs, rh) = (10.0, 100.0);
        let z2 = rh * (rs + rh) / (rh + rs + rh);
        let v2 = 5.0 * z2 / (rs + z2);
        let v3 = v2 * rh / (rs + rh);
        let v = ladder_voltages(2, rs, rh, 5.0);
        assert!((v[1] - v2).abs() < 1e-14 && (v[2] - v3).abs() < 1e-14);
        assert_eq!(v[0], 5.0);
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn ladder_recursion_matches_arbitrary_precision_values() {
        // (stages, shunt, node, value): computed offline with 80-digit arithmetic (mpmath), independent of this crate
        let table: [(usize, f64, usize, f64); 10] = [
            (100, 10_000.0, 2, 4.844_914_583_914_689_044),
            (100, 10_000.0, 50, 1.100_794_346_695_479_43),
            (100, 10_000.0, 101, 0.416_036_821_951_397_83),
            (800, 1_600_000.0, 2, 4.987_964_168_703_103_609),
            (800, 1_600_000.0, 266, 2.705_559_122_297_749_658),
            (800, 1_600_000.0, 801, 1.327_412_235_268_084_344),
            (5000, 10_000.0, 2, 4.844_366_353_991_315_308),
            (5000, 10_000.0, 300, 3.915_574_270_202_430_8e-4),
            (5000, 62_500_000.0, 2000, 2.407_121_375_484_168_905),
            (5000, 62_500_000.0, 5001, 1.328_754_969_970_841_442),
        ];
        for (n, rsh, k, exact) in table {
            let v = ladder_voltages(n, R_SER, rsh, 5.0)[k - 1];
            assert!(rel(v, exact) < 1e-13, "n={n} k={k}: {v} vs {exact}");
        }
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn ladder_recursion_error_is_below_the_stated_bound() {
        // the same 80-digit values as above: the absolute error of the f64 recursion is below the 1e-14 V that
        // `LADDER_ORACLE_NOTE` rounds up from the measured 4.4e-15 V / 8.5e-15 V (max over all 5 000 stages)
        let table: [(usize, f64, usize, f64); 6] = [
            (5000, 10_000.0, 2, 4.844_366_353_991_315_308),
            (5000, 62_500_000.0, 2000, 2.407_121_375_484_168_905),
            (5000, 62_500_000.0, 5001, 1.328_754_969_970_841_442),
            (800, 1_600_000.0, 2, 4.987_964_168_703_103_609),
            (800, 1_600_000.0, 266, 2.705_559_122_297_749_658),
            (800, 1_600_000.0, 801, 1.327_412_235_268_084_344),
        ];
        for (n, rsh, k, exact) in table {
            let v = ladder_voltages(n, R_SER, rsh, 5.0)[k - 1];
            assert!((v - exact).abs() < 1e-14, "n={n} k={k}: {v} vs {exact}");
        }
        assert!(LADDER_ORACLE_NOTE.contains("4e-15") && LADDER_ORACLE_NOTE.contains("9e-15"));
    }

    #[test]
    fn ladder_recursion_agrees_with_the_dense_oracle_to_its_own_accuracy() {
        // oracle.rs `dc_linear` is a different implementation (dense LU + compensated refinement). It is NOT more
        // accurate than the recursion on ill-conditioned ladders (6.8e-12 V off the 80-digit value at 800 flat
        // stages), so only an agreement bound proportional to the conditioning is asserted here.
        for (n, rsh, tol) in [
            (100usize, R_SH_LEGACY, 1e-12),
            (800, shunt_for_depth(800, 2.0), 1e-10),
        ] {
            let deck = ladder_deck(n, rsh);
            let truth = ladder_voltages(n, R_SER, rsh, 5.0);
            for k in [2usize, n / 3, n / 2, n] {
                let o = oracle::evaluate(&deck, "dc_linear", &format!("v({k})"), &[0.0]).unwrap();
                assert!(
                    (o[0] - truth[k - 1]).abs() <= tol * truth[k - 1].abs().max(1e-3),
                    "n={n} k={k}: oracle {} recursion {}",
                    o[0],
                    truth[k - 1]
                );
            }
        }
    }

    #[test]
    fn dense_adjoint_matches_closed_form_gradients() {
        let net = ladder_net(1, 90.0); // Rser1 = 10, Rsh1 = 90, v2 = 5 * 90 / (10 + 90)
        let g = net.gradients(2).unwrap();
        let (rs, rh, vin) = (10.0, 90.0, 5.0);
        assert!(rel(g[0], -vin * rh / ((rs + rh) * (rs + rh))) < 1e-13);
        assert!(rel(g[1], vin * rs / ((rs + rh) * (rs + rh))) < 1e-13);
    }

    #[test]
    fn central_difference_and_richardson() {
        let f = f64::exp;
        let (x, h) = (1.0, 0.1);
        let d1 = central_diff(f(x + h), f(x - h), h);
        let d2 = central_diff(f(x + h / 2.0), f(x - h / 2.0), h / 2.0);
        let exact = f(x);
        let r = richardson(d1, d2);
        assert!(
            (d1 - exact).abs() > 1e-3,
            "plain central error {}",
            (d1 - exact).abs()
        );
        assert!(
            (r - exact).abs() < 1e-5 * exact.abs(),
            "richardson error {}",
            (r - exact).abs()
        );
        assert!((r - exact).abs() < 0.01 * (d2 - exact).abs());
        // exact for a quadratic
        assert_eq!(central_diff(4.0 * 9.0, 4.0, 1.0), 16.0);
    }

    #[test]
    fn dense_adjoint_matches_richardson_finite_differences_of_the_dense_solve() {
        let n = 6;
        let net = ladder_net(n, R_SH_LEGACY);
        let target = n + 1;
        let adj = net.gradients(target).unwrap();
        let base = net.nominal();
        for (i, a) in adj.iter().enumerate() {
            let fd = |h_rel: f64| {
                let (mut p, mut m) = (base.clone(), base.clone());
                let h = base[i] * h_rel;
                p[i] += h;
                m[i] -= h;
                central_diff(
                    net.voltage(&p, target).unwrap(),
                    net.voltage(&m, target).unwrap(),
                    h,
                )
            };
            let r = richardson(fd(2e-3), fd(1e-3));
            assert!(
                (r - a).abs() <= 1e-7 * a.abs().max(1e-12),
                "param {i}: richardson {r} adjoint {a}"
            );
        }
    }

    #[test]
    fn gradient_error_metrics() {
        let r = [1.0, -2.0, 1e-6, 4.0];
        let g = [1.0, -2.0 + 2e-6, 1e-6, 4.0];
        let e = gradient_error(&g, &r).unwrap();
        assert!((e.max_abs - 2e-6).abs() < 1e-12);
        assert!((e.max_over_scale.unwrap() - 0.5e-6).abs() < 1e-12);
        // the 1e-6 entry is below 1e-3 * max and excluded from the per-gradient metric
        assert!((e.max_rel_significant.unwrap() - 1e-6).abs() < 1e-9);
        assert_eq!(e.worst_index, 1);
        assert!(gradient_error(&g[..3], &r).is_err());
        assert!(gradient_error(&[f64::NAN, 0.0, 0.0, 0.0], &r).is_err());
        let z = gradient_error(&[0.0, 0.0], &[0.0, 0.0]).unwrap();
        assert!(z.max_over_scale.is_none() && z.max_rel_significant.is_none());
    }

    #[test]
    fn table_error_aligns_labels_and_rejects_mismatches() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![2.0, 1.0 + 1e-3], vec![4.0, 3.0]];
        // swapped columns: aligned by label, not by position
        let e = table_error(&l(&["v(1)", "v(2)"]), &a, &l(&["v(2)", "v(1)"]), &b).unwrap();
        assert!((e.max_abs - 1e-3).abs() < 1e-15);
        assert_eq!((e.worst_row, e.worst_label.as_str()), (0, "v(1)"));
        assert_eq!(e.labels, 2);
        assert!(table_error(&l(&["v(1)"]), &a, &l(&["v(1)"]), &b[..1]).is_err());
        assert!(table_error(&l(&["v(9)"]), &a, &l(&["v(1)"]), &b).is_err());
        let nan = vec![vec![f64::NAN, 0.0], vec![0.0, 0.0]];
        assert!(table_error(&l(&["v(1)", "v(2)"]), &nan, &l(&["v(1)", "v(2)"]), &a).is_err());
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn parses_ngspice_console_values() {
        let out = "Doing analysis at TEMP = 27.000000\nv(101) = 4.9701542189450834e+00\n\nv(101) = 4.9701527348228582e+00\nv(2) = 1.0\nCurrent ngspice program size =   14.840 MB.\n";
        let v = parse_prints(out, "v(101)");
        assert_eq!(v, vec![4.9701542189450834, 4.9701527348228582]);
        assert_eq!(parse_prints(out, "V(2)"), vec![1.0]);
        assert!(parse_prints(out, "v(3)").is_empty());
        assert_eq!(
            parse_ng_size_kib(out),
            Some((14.840f64 * 1024.0).round() as u64)
        );
        assert_eq!(parse_ng_size_kib("nothing here"), None);
    }

    #[test]
    fn repetition_policy_and_rotation() {
        assert_eq!(effective_reps(7, 1e5), 7);
        assert_eq!(effective_reps(7, 2e6), 5);
        assert_eq!(effective_reps(7, 6e6), 3);
        assert_eq!(effective_reps(0, 1.0), 1);
        // the cap follows the contender's OWN warm-up; heavy workloads are capped at 3 regardless
        assert_eq!(contender_reps(15, false, 5e3), 15);
        assert_eq!(contender_reps(15, false, 6e6), 3);
        assert_eq!(contender_reps(15, false, 4.5e6), 5);
        assert_eq!(contender_reps(15, true, 5e3), 3);
        assert_eq!(contender_reps(2, true, 5e3), 2);
        assert_eq!(contender_reps(0, true, 5e3), 1);
        assert_eq!(rotation(3, 0), vec![0, 1, 2]);
        assert_eq!(rotation(3, 1), vec![1, 2, 0]);
        assert_eq!(rotation(3, 5), vec![2, 0, 1]);
        assert!(rotation(0, 3).is_empty());
    }

    fn sample(e2e: f64, analysis: Option<f64>) -> Sample {
        Sample {
            e2e_us: e2e,
            analysis_us: analysis,
            ..Default::default()
        }
    }

    #[test]
    fn measure_interleaves_warms_up_and_caps_heavy_workloads() {
        let order = std::cell::RefCell::new(Vec::<(String, bool)>::new());
        let mk = |name: &str, heavy: bool, e2e: f64| {
            let order = &order;
            let n = name.to_string();
            Contender {
                name: name.to_string(),
                note: String::new(),
                launches: 1,
                threads: 1,
                heavy,
                run: Box::new(move |want| {
                    order.borrow_mut().push((n.clone(), want));
                    let mut s = sample(e2e, Some(e2e / 2.0));
                    if want {
                        s.output = Some(Output::Vector(vec![e2e]));
                    }
                    Ok(s)
                }),
            }
        };
        let mut cs = vec![
            mk("a", false, 10.0),
            mk("b", false, 20.0),
            mk("h", true, 30.0),
        ];
        let ms = measure(&mut cs, 5, true);
        drop(cs);
        let log = order.borrow().clone();
        // warm-up first, in contender order, each asked for its output
        assert_eq!(
            log[..3]
                .iter()
                .map(|x| (x.0.as_str(), x.1))
                .collect::<Vec<_>>(),
            vec![("a", true), ("b", true), ("h", true)]
        );
        assert!(log[3..].iter().all(|x| !x.1));
        // rotation: repetition 1 starts with b
        assert_eq!(log[3 + 3].0, "b");
        assert_eq!(ms[0].samples.len(), 5);
        assert_eq!(ms[1].samples.len(), 5);
        assert_eq!(ms[2].samples.len(), 3, "heavy workloads are capped at 3");
        assert!(matches!(ms[0].output, Some(Output::Vector(ref v)) if v == &[10.0]));
        let st = stats_of(&ms[1]);
        assert_eq!(st.n, 5);
        assert_eq!(st.e2e_us.as_ref().unwrap().median, 20.0);
        assert_eq!(st.overhead_us.as_ref().unwrap().median, 10.0);
        assert!(st.parse_us.is_none(), "unmeasured quantities stay None");
        // no timing: warm-up only
        let mut cs = vec![mk("a", false, 10.0)];
        let ms = measure(&mut cs, 5, false);
        assert!(ms[0].samples.is_empty() && ms[0].output.is_some());
    }

    #[test]
    fn measure_reports_failures_without_aborting() {
        let mut cs = vec![
            Contender {
                name: "bad".to_string(),
                note: String::new(),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(|_| Err("boom".to_string())),
            },
            Contender {
                name: "good".to_string(),
                note: String::new(),
                launches: 1,
                threads: 1,
                heavy: false,
                run: Box::new(|_| Ok(sample(5.0, None))),
            },
        ];
        let ms = measure(&mut cs, 3, true);
        assert_eq!(ms[0].status, "failed: boom");
        assert!(ms[0].samples.is_empty());
        assert_eq!(ms[1].samples.len(), 3);
        let st = stats_of(&ms[1]);
        assert!(
            st.analysis_us.is_none() && st.overhead_us.is_none(),
            "no solve time reported => null"
        );
    }

    #[test]
    fn partial_samples_do_not_produce_a_summary() {
        // a metric reported by only some repetitions is dropped rather than summarised from a subset
        let s = [sample(1.0, Some(0.5)), sample(1.0, None)];
        assert!(summary_of(&s, |x| x.analysis_us).is_none());
        assert!(summary_of(&s, |x| Some(x.e2e_us)).is_some());
    }

    #[test]
    fn ratios_use_the_faster_ngspice_solver() {
        let r = ratio(Some(100.0), Some(60.0), Some(10.0));
        assert_eq!(r.vs_sparse, Some(10.0));
        assert_eq!(r.vs_klu, Some(6.0));
        assert_eq!(r.vs_fastest, Some(6.0));
        assert_eq!(r.fastest.as_deref(), Some("klu"));
        let r = ratio(Some(40.0), Some(60.0), Some(10.0));
        assert_eq!(
            (r.vs_fastest, r.fastest.as_deref()),
            (Some(4.0), Some("sparse"))
        );
        let r = ratio(None, Some(60.0), Some(10.0));
        assert_eq!((r.vs_sparse, r.vs_fastest), (None, Some(6.0)));
        assert_eq!(ratio(Some(1.0), Some(1.0), None), Ratio::default());
        assert_eq!(ratio(Some(1.0), Some(1.0), Some(0.0)), Ratio::default());
        assert_eq!(ratio(None, None, Some(1.0)).vs_fastest, None);
    }

    #[test]
    fn net_of_startup_ratio_is_guarded() {
        assert_eq!(
            net_ratio(60_000.0, 5_000.0, 10_000.0, 1_000.0, 0.0),
            Some(55.0 / 9.0)
        );
        assert_eq!(
            net_ratio(4_000.0, 5_000.0, 10_000.0, 1_000.0, 0.0),
            None,
            "ngspice net time <= 0"
        );
        assert_eq!(
            net_ratio(60_000.0, 5_000.0, 1_015.0, 1_000.0, 0.0),
            None,
            "RustSpice net time below 20 us"
        );
        // ER-legacy-09: a few hundred microseconds of drift between the floor and the case must not become a ratio.
        // Without the spread guard this is 240x (the raw end-to-end ratio is 5.2x); the floor spread is about 5 % of 5.7 ms
        assert_eq!(
            net_ratio(30_000.0, 18_000.0, 5_750.0, 5_700.0, 0.0),
            Some(240.0),
            "only the 20 us guard applies when no spread is given"
        );
        assert_eq!(net_ratio(30_000.0, 18_000.0, 5_750.0, 5_700.0, 285.0), None);
        assert_eq!(
            net_ratio(30_000.0, 18_000.0, 5_750.0, 5_500.0, 285.0),
            None,
            "250 us net is below 5 x 285 us"
        );
        // a net time well above 5 x the spread is kept
        let r = net_ratio(60_000.0, 5_000.0, 10_000.0, 1_000.0, 285.0).unwrap();
        assert!((r - 55.0 / 9.0).abs() < 1e-12);
        let edge = NET_SPREAD_FACTOR * 285.0;
        assert!(net_ratio(60_000.0, 5_000.0, 1_000.0 + edge - 1.0, 1_000.0, 285.0).is_none());
        assert!(net_ratio(60_000.0, 5_000.0, 1_000.0 + edge, 1_000.0, 285.0).is_some());
        assert_eq!(net_ratio(f64::NAN, 5_000.0, 10_000.0, 1_000.0, 0.0), None);
        assert_eq!(
            net_ratio(60_000.0, 5_000.0, 10_000.0, 1_000.0, f64::NAN),
            None
        );
    }

    fn cstat(name: &str, e2e: f64, analysis: Option<f64>) -> ContenderStats {
        cstat_c(name, e2e, analysis, None)
    }

    /// Stats of a contender with 3 completed repetitions; `compile` adds a RustSpice-style compile time.
    fn cstat_c(
        name: &str,
        e2e: f64,
        analysis: Option<f64>,
        compile: Option<f64>,
    ) -> ContenderStats {
        let mk = |v: f64| summarize(&[v * 1.1, v, v * 1.2]);
        ContenderStats {
            name: name.to_string(),
            status: "ok".to_string(),
            n: 3,
            reps_planned: 3,
            e2e_us: mk(e2e),
            analysis_us: analysis.and_then(mk),
            compile_us: compile.and_then(mk),
            compile_analysis_us: match (analysis, compile) {
                (Some(a), Some(c)) => summarize(&[(a + c) * 1.1, a + c, (a + c) * 1.2]),
                _ => None,
            },
            ..Default::default()
        }
    }

    #[test]
    fn speedup_set_headline_and_net() {
        let cs = vec![
            cstat(RS, 10_000.0, Some(100.0)),
            cstat(NG_SPARSE, 50_000.0, Some(5_000.0)),
            cstat(NG_KLU, 60_000.0, Some(2_000.0)),
        ];
        let floors = Floors {
            status: "ok".to_string(),
            reps: 15,
            // a floor of one sample has no spread (`stddev` is NaN for n < 2) and gives no net ratio; the real floors
            // are measured with `reps` samples
            ngspice_sparse_us: summarize(&[4990.0, 5000.0, 5010.0]),
            ngspice_klu_us: summarize(&[4990.0, 5000.0, 5010.0]),
            rustspice_us: summarize(&[999.0, 1000.0, 1001.0]),
        };
        let s = speedups(&cs, &floors);
        let close =
            |a: Option<f64>, b: f64| assert!((a.unwrap() - b).abs() < 1e-9 * b, "{a:?} vs {b}");
        close(s.e2e_median.vs_fastest, 5.0);
        assert_eq!(s.e2e_median.fastest.as_deref(), Some("sparse"));
        close(s.analysis_median.vs_fastest, 20.0);
        assert_eq!(s.analysis_median.fastest.as_deref(), Some("klu"));
        close(s.e2e_min.vs_fastest, 5.0);
        close(s.e2e_net_of_startup, 45_000.0 / 9_000.0);
        // no floors measured -> null, not a guess
        let none = speedups(&cs, &Floors::default());
        assert_eq!(none.e2e_net_of_startup, None);
        assert_eq!(noisy_names(&cs), Vec::<String>::new());
    }

    #[test]
    fn finite_difference_decks() {
        let net = ladder_net(3, 1e4);
        let perts = fd_perturbations(&net, 1e-4);
        assert_eq!(perts.len(), 6);
        for ((p, m), r) in perts.iter().zip(net.res.iter()) {
            assert!(*p > r.3 && *m < r.3);
            // the decimal text written to the deck is exactly the perturbed double
            assert_eq!(num(*p).parse::<f64>().unwrap(), *p);
            assert_eq!(num(*m).parse::<f64>().unwrap(), *m);
        }
        let deck = fd_control_deck(&net, 1e4, 4, &perts);
        assert_eq!(
            deck.matches("\nop\n").count(),
            12,
            "two operating points per resistor"
        );
        assert_eq!(deck.matches("print v(4)").count(), 12);
        assert_eq!(
            deck.matches("alter ").count(),
            18,
            "perturb +, perturb -, restore"
        );
        assert!(deck.contains(".control\nset numdgt=16\n") && deck.contains(".endc\n.end\n"));
        assert!(deck.contains(&format!("alter Rser1 = {}\n", num(perts[0].0))));
    }

    #[test]
    fn corner_values_and_cards() {
        let v = corner_values(128);
        assert_eq!(v.len(), 128);
        assert_eq!(v[0], 1.5);
        assert!((v[127] - (1.5 + 0.03 * 127.0)).abs() < 1e-15);
        let card = corner_sweep_card(128);
        let toks: Vec<&str> = card.split_whitespace().collect();
        assert_eq!(toks[0], ".dc");
        assert_eq!(toks[1], "V1");
        let (start, stop, step) = (
            toks[2].parse::<f64>().unwrap(),
            toks[3].parse::<f64>().unwrap(),
            toks[4].parse::<f64>().unwrap(),
        );
        assert_eq!((start, step), (1.5, 0.03));
        let last = v[127];
        assert!(
            stop > last && stop < last + 0.5 * step,
            "stop {stop} must not add or drop a point"
        );
        let ctl = corner_control_deck(4, &corner_values(3), Path::new("/dev/shm/x.raw"));
        assert_eq!(ctl.matches("\nop\n").count(), 3);
        assert_eq!(ctl.matches("write /dev/shm/x.raw all").count(), 3);
        assert!(ctl.contains("set appendwrite"));
    }

    fn plot(name: &str, vars: &[&str], cols: Vec<Vec<f64>>) -> Plot {
        Plot {
            title: "t".to_string(),
            plotname: name.to_string(),
            vars: vars
                .iter()
                .map(|v| Var {
                    name: v.to_string(),
                    kind: "voltage".to_string(),
                })
                .collect(),
            npoints: cols[0].len(),
            cols,
        }
    }

    #[test]
    fn sensitivity_plot_is_read_by_resistor_name() {
        let p = plot(
            "Sensitivity Analysis",
            &["v(rsh1:r)", "v(rser1)", "v(rsh1)"],
            vec![vec![0.0], vec![-1.5e-3], vec![4.9e-7]],
        );
        let g = sens_gradients(
            std::slice::from_ref(&p),
            &["Rser1".to_string(), "Rsh1".to_string()],
        )
        .unwrap();
        assert_eq!(g, vec![-1.5e-3, 4.9e-7]);
        assert!(sens_gradients(std::slice::from_ref(&p), &["Rx".to_string()]).is_err());
        let op = plot("Operating Point", &["v(1)"], vec![vec![1.0]]);
        assert!(sens_gradients(&[op], &["Rser1".to_string()]).is_err());
    }

    #[test]
    fn tables_from_plots() {
        let dc = plot(
            "DC transfer characteristic",
            &["v(v-sweep)", "v(1)", "i(v1)", "v(2)"],
            vec![
                vec![0.0, 1.0],
                vec![0.0, 1.0],
                vec![0.0, -1.0],
                vec![0.0, 0.5],
            ],
        );
        let (labels, rows) = table_from_dc(&[dc]).unwrap();
        assert_eq!(labels, vec!["v(1)".to_string(), "v(2)".to_string()]);
        assert_eq!(rows, vec![vec![0.0, 0.0], vec![1.0, 0.5]]);
        let op = |a: f64, b: f64| {
            plot(
                "Operating Point",
                &["v(1)", "v(2)", "i(v1)"],
                vec![vec![a], vec![b], vec![0.0]],
            )
        };
        let t = table_from_ops(
            &[op(1.0, 2.0), op(3.0, 4.0)],
            &["v(2)".to_string(), "v(1)".to_string()],
        )
        .unwrap();
        assert_eq!(t, vec![vec![2.0, 1.0], vec![4.0, 3.0]]);
        assert!(table_from_ops(&[op(1.0, 2.0)], &["v(9)".to_string()]).is_err());
        let p = dc_plot_from_table(&[0.0, 1.0], &labels, &rows);
        assert_eq!(p.kind(), Some(PlotKind::Dc));
        assert_eq!(p.col("v(2)").unwrap(), &[0.0, 0.5]);
    }

    fn fake_case(id: &str, size: usize, e2e_scale: f64) -> CaseResult {
        let spec = CaseSpec {
            id: id.to_string(),
            bench: "b1",
            circuit: String::new(),
            detail: String::new(),
            size,
            legacy: false,
            kind: PlotKind::Op,
            deck: ladder_deck(2, 1e4),
            floor: 1e-6,
            oracle: None,
            ladder: None,
            ref_tmax: None,
            ref_check: false,
        };
        let mut c = CaseResult::new(&spec);
        let t = size as f64 * e2e_scale;
        c.contenders = vec![
            cstat(RS, t, Some(t / 10.0)),
            cstat(NG_SPARSE, 4.0 * t, Some(t)),
            cstat(NG_KLU, 5.0 * t, Some(2.0 * t)),
        ];
        c.speedups = Some(speedups(&c.contenders, &Floors::default()));
        c.accuracy = Accuracy {
            status: "ok".to_string(),
            metric: "max |dV| (V)".to_string(),
            rs_err: Some(1e-12),
            ng_err: Some(1e-5),
            ..Default::default()
        };
        c.rustspice_ok = true;
        c
    }

    #[test]
    fn families_and_scaling_exponents() {
        assert_eq!(family_of("b1_ladder_5000"), "b1_ladder");
        assert_eq!(family_of("b1n_ladder_flat_500"), "b1n_ladder_flat");
        assert_eq!(family_of("noindex"), "noindex");
        let cases = vec![
            fake_case("b1_ladder_100", 100, 1.0),
            fake_case("b1_ladder_1000", 1000, 1.0),
            fake_case("b1_ladder_10000", 10000, 1.0),
            fake_case("b2s_rcline_500_ts1p", 500, 1.0),
        ];
        let rows = scaling_rows(&cases);
        assert!(
            rows.iter().all(|r| r.family == "b1_ladder"),
            "series members are excluded"
        );
        let rs = rows
            .iter()
            .find(|r| r.contender == RS && r.metric == "e2e")
            .unwrap();
        assert!((rs.fit.exponent.unwrap() - 1.0).abs() < 1e-9);
        let two = scaling_rows(&cases[..2]);
        assert!(
            two.iter().all(|r| r.fit.exponent.is_none()),
            "two points are not a fit"
        );
    }

    #[test]
    fn markdown_tables_are_rectangular() {
        let t = md_table(
            &["a", "b", "c"],
            &[vec!["1".into(), "x|y".into()], vec!["1".into(); 5]],
        );
        for l in t.lines() {
            let cells = l.replace("\\|", "").matches('|').count();
            assert_eq!(cells, 4, "{l}");
        }
        assert!(t.contains("x\\|y"));
        assert!(
            t.lines().nth(2).unwrap().ends_with("| n/a |"),
            "short rows are padded with n/a"
        );
    }

    fn fake_report() -> Report {
        let mut cases = vec![fake_case("b1_ladder_5000", 5000, 0.001)];
        cases[0].legacy_size = true;
        cases[0].detail = "10,000 resistors".to_string();
        let work = |legacy: bool| WorkResult {
            id: "w".to_string(),
            bench: "b5".to_string(),
            title: "title".to_string(),
            detail: "detail".to_string(),
            size: 200,
            legacy_size: legacy,
            reference: "ref".to_string(),
            reference_status: "ok".to_string(),
            rs_baseline: "rustspice-adjoint".to_string(),
            contenders: vec![
                WorkContender {
                    stats: cstat("rustspice-adjoint", 1500.0, Some(80.0)),
                    gradient: Some(GradError {
                        n: 2,
                        max_abs: 1e-16,
                        max_over_scale: Some(1e-14),
                        max_rel_significant: Some(1e-13),
                        worst_index: 0,
                    }),
                    accuracy_status: "ok".to_string(),
                    e2e_ratio_vs_rs: Some(1.0),
                    ..Default::default()
                },
                WorkContender {
                    stats: cstat("ngspice-sens-sparse", 19_000.0, Some(14_000.0)),
                    gradient: None,
                    accuracy_status: "n/a: no output".to_string(),
                    e2e_ratio_vs_rs: Some(12.7),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        Report {
            schema: SCHEMA,
            tool: "t".to_string(),
            elapsed_s: 1.0,
            parent_peak_rss_kib: Some(10_000),
            host: HostInfo {
                cpu: "cpu".to_string(),
                threads: 4,
                hardware_threads: Some(32),
                loadavg_start: Some(1.0),
                loadavg_end: None,
                pin_cpu: None,
                tmp_on_tmpfs: true,
            },
            ngspice: NgInfo {
                version: Some("ngspice-47".to_string()),
                path: "ngspice".to_string(),
                installed: true,
                solvers_timed: vec!["sparse".to_string(), "klu".to_string()],
            },
            rustspice: RsInfo {
                version: "0.1.0".to_string(),
                git: None,
                git_dirty: None,
                source_tree: "/src/rustspice-cli".to_string(),
                build_profile: "release".to_string(),
            },
            scope: ScopeInfo {
                temperature_c: 27.0,
                ngspice_asis_tolerances: "defaults".to_string(),
                ngspice_reference_op_dc: "tight".to_string(),
                ngspice_reference_tran: "tran".to_string(),
                rustspice_tolerances: "fixed".to_string(),
                step_policy: "fixed".to_string(),
                reps_requested: 7,
                timing_enabled: true,
                tran_floor_v: TRAN_FLOOR_V,
            },
            floors: Floors::default(),
            scaling: scaling_rows(&cases),
            cases,
            series: Vec::new(),
            sensitivity: vec![work(true)],
            batch: Vec::new(),
            previously_published: PUBLISHED.to_vec(),
        }
    }

    #[test]
    fn report_renders_without_inventing_numbers() {
        let r = fake_report();
        let md = render_markdown(&r);
        assert!(md.contains("# Benchmarks: RustSpice vs. ngspice-47"));
        assert!(md.contains("previously published"));
        assert!(md.contains("n/a"), "unmeasured entries are n/a");
        assert!(!md.contains("NaN") && !md.contains("inf"), "{md}");
        // every table block is rectangular
        let mut width: Option<usize> = None;
        for l in md.lines() {
            if l.starts_with('|') {
                let w = l.replace("\\|", "").matches('|').count();
                match width {
                    Some(x) => assert_eq!(x, w, "ragged table row: {l}"),
                    None => width = Some(w),
                }
            } else {
                width = None;
            }
        }
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["schema"], 1);
        // unmeasured quantities are null, never 0
        assert!(j["host"]["loadavg_end"].is_null());
        assert!(j["floors"]["rustspice_us"].is_null());
        assert!(j["sensitivity"][0]["contenders"][1]["gradient"].is_null());
    }

    #[test]
    fn missing_ngspice_is_stated_in_the_report() {
        let mut r = fake_report();
        r.ngspice.installed = false;
        r.ngspice.version = None;
        let md = render_markdown(&r);
        assert!(md.contains("ngspice is not installed"));
        assert!(md.contains("unverified"));
        let a = Accuracy::na("ngspice is not installed (unverified)");
        let j = serde_json::to_value(&a).unwrap();
        assert!(j["rs_err"].is_null() && j["ng_err"].is_null());
        assert!(a.status.starts_with("n/a"));
    }

    #[test]
    fn published_numbers_are_self_consistent() {
        for p in PUBLISHED.iter() {
            let e2e = p.ng_wall_ms / p.rs_total_ms;
            let solver = p.ng_solver_ms / p.rs_solver_ms;
            assert!(
                rel(e2e, p.e2e_speedup) < 0.01,
                "{}: {e2e} vs {}",
                p.id,
                p.e2e_speedup
            );
            assert!(
                rel(solver, p.solver_speedup) < 0.02,
                "{}: {solver} vs {}",
                p.id,
                p.solver_speedup
            );
        }
        assert_eq!(PUBLISHED.len(), 6);
        // the derived "single ngspice run" of b5 / b6 used in the report text
        assert!((PUBLISHED[4].ng_wall_ms / 400.0 - 5.418).abs() < 0.01);
        assert!((PUBLISHED[5].ng_wall_ms / 128.0 - 5.36).abs() < 0.01);
    }

    #[test]
    fn reference_decks_use_the_documented_options() {
        let tran = rc_line_deck(4, "10p");
        let (d, label) = reference_deck(&tran, PlotKind::Tran, None, None).unwrap();
        assert!(d.contains(&format!(".options {TRAN_REF_OPTIONS}")));
        let card = meta::find_tran(&d).unwrap();
        assert_eq!(card.tmax, Some(0.5e-12), "tmax = tstep / 20");
        assert!(label.contains("reltol=1e-6"));
        let (d, _) =
            reference_deck(&tran, PlotKind::Tran, Some(1e-13), Some("reltol=1e-7")).unwrap();
        assert!(
            d.contains(".options reltol=1e-7") && meta::find_tran(&d).unwrap().tmax == Some(1e-13)
        );
        let (d, _) = reference_deck(&ladder_deck(2, 1e4), PlotKind::Op, None, None).unwrap();
        assert!(d.contains(&format!(".options {TIGHT_OPTIONS}")));
        assert!(reference_deck(&ladder_deck(2, 1e4), PlotKind::Tran, None, None).is_err());
    }

    #[test]
    fn case_list_covers_the_six_benchmarks() {
        let all = case_specs(&[]);
        for b in ["b1", "b2", "b3", "b4"] {
            let sizes: Vec<usize> = all
                .iter()
                .filter(|c| c.bench == b && c.id.starts_with(&format!("{b}_")))
                .map(|c| c.size)
                .collect();
            assert!(sizes.len() >= 3, "{b}: size ladder {sizes:?}");
            assert_eq!(
                all.iter().filter(|c| c.bench == b && c.legacy).count(),
                1,
                "{b}"
            );
        }
        let legacy: Vec<usize> = all.iter().filter(|c| c.legacy).map(|c| c.size).collect();
        assert_eq!(legacy, vec![5000, 2000, 64, 100], "the published sizes");
        assert_eq!(
            case_specs(&["b3".to_string()])
                .iter()
                .filter(|c| c.bench != "b3")
                .count(),
            0
        );
        assert!(all.iter().all(|c| c.deck.lines().next().is_some()));
        let mut ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "case ids are unique");
    }

    #[test]
    fn series_equal_error_needs_overlap() {
        let pt = |t: f64, re: f64, ne: f64| SeriesPoint {
            tstep_s: t,
            rs_err: Some(re),
            ng_err: Some(ne),
            rs_e2e_us: Some(1.0 / re),
            ng_e2e_us: Some(4.0 / ne),
            rs_analysis_us: None,
            ng_analysis_us: None,
            rs_compile_analysis_us: None,
        };
        // disjoint error ranges: no interpolation is attempted
        let (e2e, analysis, lfl) =
            series_equal_error(&[pt(1e-11, 1e-1, 4e-3), pt(1e-12, 1e-2, 1e-3)]);
        assert_eq!(e2e.unwrap().status, "no_overlap");
        assert!(
            analysis.is_none() && lfl.is_none(),
            "no analysis times measured"
        );
        // overlapping ranges interpolate (time = 1/err for RustSpice, 4/err for ngspice => ratio 4)
        let (e2e, _, _) = series_equal_error(&[
            pt(1e-11, 1e-1, 1e-1),
            pt(1e-12, 1e-3, 1e-3),
            pt(1e-13, 1e-5, 1e-5),
        ]);
        let e = e2e.unwrap();
        assert_eq!(e.status, "ok");
        assert!(e
            .levels
            .iter()
            .all(|l| (l.ratio_b_over_a - 4.0).abs() < 1e-6));
    }

    // ---- ER-legacy hardening: honest columns, valid headline contenders, per-contender repetition caps

    fn measured(name: &str, samples: Vec<Sample>) -> Measured {
        Measured {
            name: name.to_string(),
            note: String::new(),
            launches: 1,
            threads: 1,
            status: "ok".to_string(),
            info: None,
            warm_e2e_us: samples.first().map(|s| s.e2e_us),
            reps_planned: samples.len(),
            output: None,
            samples,
        }
    }

    #[test]
    fn a_slow_light_contender_caps_only_itself() {
        // ER-legacy-07: with --reps 15 the fast contender must keep 15 repetitions although a light neighbour has a 6 s
        // warm-up (it used to cap every light contender at 3)
        let mk = |name: &str, warm: f64, e2e: f64, heavy: bool| Contender {
            name: name.to_string(),
            note: String::new(),
            launches: 1,
            threads: 1,
            heavy,
            run: Box::new(move |want| Ok(sample(if want { warm } else { e2e }, None))),
        };
        let mut cs = vec![
            mk("fast", 5e3, 5e3, false),
            mk("slowlight", 6e6, 6e6, false),
            mk("midlight", 4.5e6, 4.5e6, false),
            mk("heavy", 5e3, 5e3, true),
        ];
        let ms = measure(&mut cs, 15, true);
        let n: Vec<usize> = ms.iter().map(|m| m.samples.len()).collect();
        assert_eq!(n, vec![15, 3, 5, 3]);
        let planned: Vec<usize> = ms.iter().map(|m| m.reps_planned).collect();
        assert_eq!(planned, n);
        assert!(ms.iter().all(|m| stats_of(m).completed()));
        // timing off: nothing scheduled, nothing completed
        let ms = measure(&mut cs, 15, false);
        assert!(ms
            .iter()
            .all(|m| m.reps_planned == 0 && m.samples.is_empty()));
        assert!(ms.iter().all(|m| !stats_of(m).completed()));
    }

    #[test]
    fn a_failed_repetition_leaves_the_contender_incomplete() {
        let mut calls = 0;
        let mut cs = vec![Contender {
            name: "flaky".to_string(),
            note: String::new(),
            launches: 1,
            threads: 1,
            heavy: false,
            run: Box::new(move |_| {
                calls += 1;
                if calls == 4 {
                    Err("timeout".to_string())
                } else {
                    Ok(sample(1000.0, None))
                }
            }),
        }];
        let ms = measure(&mut cs, 7, true);
        assert_eq!(ms[0].status, "failed in repetition 3: timeout");
        assert_eq!((ms[0].samples.len(), ms[0].reps_planned), (2, 7));
        let st = stats_of(&ms[0]);
        assert!(!st.completed(), "2 of 7 repetitions is not a result");
        assert_eq!(n_cell(Some(&st)), "2 of 7");
        assert_eq!(time_cell(Some(&st), |s| &s.e2e_us), st.status);
    }

    #[test]
    fn workload_reported_threads_replace_the_assumed_count() {
        // ER-legacy-11: the table shows the worker's own thread count, not available_parallelism of the parent
        let mk = |threads: Option<usize>| Contender {
            name: "w".to_string(),
            note: String::new(),
            launches: 1,
            threads: 16,
            heavy: false,
            run: Box::new(move |_| {
                Ok(Sample {
                    threads,
                    ..sample(1.0, None)
                })
            }),
        };
        let ms = measure(&mut [mk(Some(1))], 3, true);
        assert_eq!((ms[0].threads, stats_of(&ms[0]).threads), (1, 1));
        let ms = measure(&mut [mk(None)], 3, true);
        assert_eq!(
            ms[0].threads, 16,
            "tools that do not report keep the harness value"
        );
    }

    #[test]
    fn like_for_like_ratio_counts_rustspice_compile_time() {
        // ER-legacy-02, b1_ladder_5000 of the audit: ngspice analysis 22 268 us; RustSpice analysis 380 us, compile
        // 24 736 us. Analysis-only says 58.6x, the like-for-like ratio is 0.89x (RustSpice slower).
        let rs_s = |e2e: f64| Sample {
            e2e_us: e2e,
            analysis_us: Some(380.0),
            compile_us: Some(24_736.0),
            parse_us: Some(14_378.0),
            ..Default::default()
        };
        let ng_s = |e2e: f64| sample(e2e, Some(22_268.0));
        let rs = stats_of(&measured(RS, vec![rs_s(40_000.0); 3]));
        let ng = stats_of(&measured(NG_SPARSE, vec![ng_s(60_000.0); 3]));
        assert_eq!(rs.compile_analysis_us.as_ref().unwrap().median, 25_116.0);
        assert!(
            ng.compile_analysis_us.is_none(),
            "ngspice has no compile time"
        );
        let sp = speedups(&[rs, ng], &Floors::default());
        let x = |r: &Ratio| r.vs_sparse.unwrap();
        assert!((x(&sp.e2e_median) - 1.5).abs() < 1e-12);
        assert!((x(&sp.analysis_median) - 22_268.0 / 380.0).abs() < 1e-9);
        assert!((x(&sp.like_for_like_median) - 22_268.0 / 25_116.0).abs() < 1e-9);
        assert!(x(&sp.like_for_like_median) < 1.0 && x(&sp.analysis_median) > 50.0);
        // a RustSpice run without a compile time has no like-for-like ratio (never 0 or the analysis-only value)
        let mut rs2 = measured(RS, vec![rs_s(40_000.0); 3]);
        for s in &mut rs2.samples {
            s.compile_us = None;
        }
        let sp = speedups(
            &[
                stats_of(&rs2),
                stats_of(&measured(NG_SPARSE, vec![ng_s(60_000.0); 3])),
            ],
            &Floors::default(),
        );
        assert!(sp.analysis_median.vs_sparse.is_some());
        assert_eq!(sp.like_for_like_median, Ratio::default());
    }

    #[test]
    fn a_contender_that_failed_a_repetition_never_enters_a_ratio() {
        let mut bad = cstat(NG_SPARSE, 10_000.0, Some(5_000.0));
        bad.status = "failed in repetition 3: timeout".to_string();
        bad.n = 2;
        let cs = vec![
            cstat(RS, 1_000.0, Some(100.0)),
            bad,
            cstat(NG_KLU, 4_000.0, Some(2_000.0)),
        ];
        let sp = speedups(&cs, &Floors::default());
        assert_eq!(sp.e2e_median.vs_sparse, None);
        assert!((sp.e2e_median.vs_klu.unwrap() - 4.0).abs() < 1e-12);
        assert_eq!(sp.e2e_median.fastest.as_deref(), Some("klu"));
        // a RustSpice contender with a missing repetition leaves every ratio empty
        let mut cs2 = cs.clone();
        cs2[0].n = 2;
        let sp = speedups(&cs2, &Floors::default());
        assert_eq!(sp.e2e_median, Ratio::default());
        assert_eq!(sp.like_for_like_median, Ratio::default());
    }

    /// A b5/b6 contender with `n` completed repetitions and an accuracy figure (`None`: n/a).
    fn wc(name: &str, e2e: f64, accuracy: Option<f64>) -> WorkContender {
        let mut stats = cstat(name, e2e, None);
        stats.launches = 1;
        stats.threads = 1;
        stats.e2e_us = summarize(&[e2e, e2e, e2e]);
        WorkContender {
            gradient: accuracy.map(|g| GradError {
                n: 2,
                max_abs: g,
                max_over_scale: Some(g),
                max_rel_significant: Some(g),
                worst_index: 0,
            }),
            accuracy_status: if accuracy.is_some() {
                "ok".to_string()
            } else {
                "n/a: no output".to_string()
            },
            stats,
            ..Default::default()
        }
    }

    #[test]
    fn best_ngspice_only_considers_complete_contenders_with_a_valid_accuracy() {
        // ER-legacy-06: a contender that failed in repetition 3 (2 samples, 2.0-2.1 ms) or has no accuracy figure
        // used to become the "best ngspice variant"
        let mut failed = wc("ngspice-fd-loop-sparse", 2_050.0, None);
        failed.stats.status = "failed in repetition 3: timeout".to_string();
        failed.stats.n = 2;
        let no_acc = wc("ngspice-fd-loop-klu", 2_100.0, None);
        let mut partial = wc("ngspice-fd-launches-central", 2_200.0, Some(1e-9));
        partial.stats.n = 2;
        let w = WorkResult {
            size: 200,
            legacy_size: true,
            contenders: vec![
                wc("rustspice-adjoint", 1_500.0, Some(1e-14)),
                failed,
                no_acc,
                partial,
                wc("ngspice-sens-sparse", 19_000.0, Some(1e-6)),
                wc("ngspice-fd-launches-forward", 30_000.0, Some(2e-5)),
            ],
            ..Default::default()
        };
        assert_eq!(best_ngspice(&w).unwrap().stats.name, "ngspice-sens-sparse");
        assert_eq!(
            best_ngspice_requested_only(&w).unwrap().stats.name,
            "ngspice-fd-launches-forward"
        );
        assert_eq!(best_rustspice(&w).unwrap().stats.name, "rustspice-adjoint");
        let row = sens_headline_row(&w);
        assert!(
            row[0].contains("best ngspice variant: ngspice-sens-sparse"),
            "{row:?}"
        );
        // the failed contender is flagged in the row (status is printed) but is never the headline variant
        assert!(
            row[9].contains("ngspice-fd-loop-sparse: failed in repetition 3: timeout"),
            "{row:?}"
        );
        assert!(
            !row[0].contains("fd-loop") && !row[3].contains("fd-loop"),
            "{row:?}"
        );
        // nothing valid: no best variant, no ratio
        let w = WorkResult {
            contenders: vec![
                wc("rustspice-adjoint", 1_500.0, Some(1e-14)),
                wc("ngspice-sens-sparse", 19_000.0, None),
            ],
            ..Default::default()
        };
        assert!(best_ngspice(&w).is_none());
        let row = sens_headline_row(&w);
        assert_eq!(
            (row[2].as_str(), row[3].as_str()),
            ("n/a", "n/a"),
            "{row:?}"
        );
    }

    #[test]
    fn finish_work_gives_partial_contenders_no_ratio() {
        let rs = measured("rs", vec![sample(100.0, Some(10.0)); 3]);
        let ng = measured("ng", vec![sample(500.0, Some(50.0)); 3]);
        let mut bad = measured("bad", vec![sample(300.0, Some(30.0)); 2]);
        bad.reps_planned = 3;
        bad.status = "failed in repetition 3: timeout".to_string();
        let ms = [rs, ng, bad];
        let (c, reps) = finish_work(&ms, "rs", true, |_| (None, None, "ok".to_string()));
        assert_eq!(reps, 3);
        assert_eq!(c[1].e2e_ratio_vs_rs, Some(5.0));
        assert_eq!(c[1].analysis_ratio_vs_rs, Some(5.0));
        assert_eq!(c[2].e2e_ratio_vs_rs, None);
        assert!(c[1].valid() && !c[2].valid());
    }

    fn headline_report() -> Report {
        let mut r = fake_report();
        r.sensitivity.clear();
        r
    }

    fn legacy_fake(bench: &'static str, analysis: &str, size: usize) -> CaseResult {
        let mut c = fake_case(&format!("{bench}_x_{size}"), size, 1.0);
        c.bench = bench.to_string();
        c.analysis = analysis.to_string();
        c.legacy_size = true;
        c.detail = "detail".to_string();
        c
    }

    #[test]
    fn headline_accuracy_shows_the_oracle_not_only_the_reference() {
        // ER-legacy-05, b3 64 diodes: RustSpice 5.9e-11 V against the oracle, but 6.8e-10 V against the tight
        // reference, which has that error itself. The headline must not say RustSpice is 11x less accurate.
        let mut c = legacy_fake("b3", "dc", 64);
        c.accuracy = Accuracy {
            status: "ok".to_string(),
            rs_err: Some(6.838e-10),
            ng_err: Some(5.443e-5),
            oracle: Some("dc_nonlinear".to_string()),
            oracle_status: Some("ok".to_string()),
            rs_vs_oracle: Some(5.894e-11),
            ng_vs_oracle: Some(5.443e-5),
            ref_vs_oracle: Some(6.838e-10),
            ..Default::default()
        };
        let row = case_headline_row(&headline_report(), &c, "b3 diode sweep");
        assert!(
            row[6].starts_with("5.89e-11 / 5.44e-5 V")
                && row[6].contains("tight reference itself: 6.84e-10"),
            "{}",
            row[6]
        );
        assert_eq!(row[7], "6.84e-10 / 5.44e-5");
        assert!(
            !row[3].contains("unequal accuracy"),
            "RustSpice is the more accurate tool"
        );
        // the judge of "RustSpice <= ngspice" is the oracle when there is one
        let (rs, ng, basis) = accuracy_pair(&c.accuracy).unwrap();
        assert_eq!((rs, ng, basis), (5.894e-11, 5.443e-5, "independent oracle"));
        // without an oracle the reference is the (labelled) fallback
        c.accuracy.oracle = None;
        c.accuracy.oracle_status = None;
        let row = case_headline_row(&headline_report(), &c, "b3");
        assert!(row[6].starts_with("n/a (no oracle"), "{}", row[6]);
        assert_eq!(accuracy_pair(&c.accuracy).unwrap().2, "tight reference");
        // a zero distance is "identical", never a bare 0 error
        assert_eq!(fmt_agree(Some(0.0)), "0 (identical to the reference)");
        assert_eq!(fmt_agree(Some(1.5e-13)), "1.50e-13");
        assert_eq!(fmt_agree(None), "n/a");
    }

    #[test]
    fn b2_headline_carries_its_accuracy_qualifier_in_the_same_row() {
        // ER-legacy-03: 5x faster at 58x the error and with no equal-error overlap
        let mut c = legacy_fake("b2", "tran", 2000);
        c.accuracy = Accuracy {
            status: "ok".to_string(),
            rs_err: Some(9.27e-2),
            ng_err: Some(1.6e-3),
            ..Default::default()
        };
        let mut r = headline_report();
        r.series = vec![SeriesResult {
            id: "b2s_rcline_500".to_string(),
            title: "t".to_string(),
            points: Vec::new(),
            equal_error_e2e: Some(EqualError {
                levels: Vec::new(),
                range: None,
                status: "no_overlap".to_string(),
                a_range: Some((9.3e-3, 3.1e-1)),
                b_range: Some((1.5e-3, 5.3e-3)),
                a_dropped: 0,
                b_dropped: 0,
                a_zero_error: 0,
                b_zero_error: 0,
            }),
            equal_error_analysis: None,
            equal_error_like_for_like: None,
        }];
        let row = case_headline_row(&r, &c, "b2 RC line tran");
        assert!(row[3].ends_with("; unequal accuracy"), "{}", row[3]);
        assert!(
            row[10].contains("RustSpice error is 57.9x ngspice default's"),
            "{}",
            row[10]
        );
        assert!(
            row[10]
                .contains("no equal-error comparison exists on the step ladder `b2s_rcline_500`"),
            "{}",
            row[10]
        );
        assert!(
            row[10].contains("does not reach ngspice's error"),
            "{}",
            row[10]
        );
        // an equal-error table, when it exists, is quoted instead
        r.series[0].equal_error_e2e = Some(EqualError {
            levels: vec![EqualErrorLevel {
                err: 1e-2,
                t_a: 2.0,
                t_b: 1.0,
                ratio_b_over_a: 0.5,
            }],
            range: Some((5e-3, 2e-2)),
            status: "ok".to_string(),
            a_range: None,
            b_range: None,
            a_dropped: 0,
            b_dropped: 0,
            a_zero_error: 0,
            b_zero_error: 0,
        });
        let row = case_headline_row(&r, &c, "b2");
        assert!(
            row[10].contains("equal-error end-to-end ratio (ngspice / RustSpice)"),
            "{}",
            row[10]
        );
        assert!(row[10].contains("0.50x"), "{}", row[10]);
    }

    #[test]
    fn unequal_accuracy_flag_thresholds() {
        let acc = |rs: f64, ng: f64| Accuracy {
            status: "ok".to_string(),
            rs_err: Some(rs),
            ng_err: Some(ng),
            ..Default::default()
        };
        assert!(unequal_accuracy_text(&acc(1.0e-9, 1.0e-9)).is_none());
        assert!(unequal_accuracy_text(&acc(1.9e-9, 1.0e-9)).is_none());
        let t = unequal_accuracy_text(&acc(2.0e-9, 1.0e-9)).unwrap();
        assert!(
            t.contains("2.00x ngspice default's") && t.contains("tight reference"),
            "{t}"
        );
        assert!(unequal_accuracy_text(&acc(1e-12, 1e-6)).is_none());
        let t = unequal_accuracy_text(&acc(1e-9, 0.0)).unwrap();
        assert!(t.contains("identical"), "{t}");
        assert!(unequal_accuracy_text(&acc(0.0, 0.0)).is_none());
        assert!(unequal_accuracy_text(&Accuracy::na("x")).is_none());
        let mut a = acc(1e-9, 1e-9);
        a.oracle_status = Some("ok".to_string());
        a.rs_vs_oracle = Some(1e-7);
        a.ng_vs_oracle = Some(1e-8);
        let t = unequal_accuracy_text(&a).unwrap();
        assert!(
            t.contains("10.0x") && t.contains("independent oracle"),
            "{t}"
        );
    }

    #[test]
    fn b5_headline_states_what_sens_computes_and_gives_the_same_output_ratio() {
        // ER-legacy-04: `.sens` writes 4805 vectors for the 200 requested gradients; the ratio against the fastest
        // variant that delivers only the requested gradients is shown next to it
        let w = WorkResult {
            size: 200,
            legacy_size: true,
            sens_vectors: Some(4805),
            contenders: vec![
                wc("rustspice-adjoint", 2_000.0, Some(1e-14)),
                wc("ngspice-sens-sparse", 20_000.0, Some(1e-6)),
                wc("ngspice-fd-loop-sparse", 1_500_000.0, Some(1e-9)),
            ],
            ..Default::default()
        };
        let row = sens_headline_row(&w);
        assert!(
            row[3].starts_with(
                "10.0x (min 10.0x) vs ngspice-sens-sparse (`.sens` computes 24.0 sensitivities per requested gradient); "
            ),
            "{}",
            row[3]
        );
        assert!(
            row[3].contains("750x vs ngspice-fd-loop-sparse (the fastest variant delivering only the 200 requested gradients)"),
            "{}",
            row[3]
        );
        assert!(
            row[10].contains("4805 sensitivity vectors for the 200 requested gradients"),
            "{}",
            row[10]
        );
        assert!(row[10].contains("gradient error is 1.00e-6"), "{}", row[10]);
        assert!(
            row[10].contains("ngspice-fd-loop-sparse) has gradient error 1.00e-9"),
            "{}",
            row[10]
        );
        assert!(
            row[6].contains("whose own error is about 1.2e-13"),
            "{}",
            row[6]
        );
        assert_eq!(row[8], "3 / 3");
        // when the best ngspice variant is a finite-difference loop, no `.sens` caveat and no second ratio
        let w2 = WorkResult {
            contenders: vec![
                wc("rustspice-adjoint", 2_000.0, Some(1e-14)),
                wc("ngspice-sens-sparse", 20_000_000.0, Some(1e-6)),
                wc("ngspice-fd-loop-sparse", 1_500_000.0, Some(1e-9)),
            ],
            ..w
        };
        let row = sens_headline_row(&w2);
        assert!(!row[3].contains(';') && row[10] == "-", "{row:?}");
    }

    #[test]
    fn b6_headline_shows_the_best_rustspice_path_and_the_thread_counts() {
        // ER-legacy-12: the batch API (all threads) is slower than one ngspice .dc sweep, but RustSpice's own best path
        // is faster than ngspice's best path
        let tbl = |m: f64| TableError {
            rows: 128,
            labels: 33,
            max_abs: m,
            max_rel: m,
            worst_row: 0,
            worst_label: "v(1)".to_string(),
        };
        let mut batch = wc("rustspice-batch", 22_060.0, None);
        batch.stats.threads = 2;
        batch.corners = Some(tbl(6.76e-10));
        batch.accuracy_status = "ok".to_string();
        let mut dc = wc("rustspice-dc-sweep", 7_360.0, None);
        dc.corners = Some(tbl(6.76e-10));
        dc.accuracy_status = "ok".to_string();
        let mut sweep = wc("ngspice-dc-sweep-sparse", 18_950.0, None);
        sweep.corners = Some(tbl(5e-5));
        sweep.accuracy_status = "ok".to_string();
        let mut seq = wc("ngspice-seq-launches", 700_000.0, None);
        seq.corners = Some(tbl(5e-5));
        seq.accuracy_status = "ok".to_string();
        let w = WorkResult {
            size: 128,
            legacy_size: true,
            oracle: Some(OracleBlock {
                spec: "dc_nonlinear".to_string(),
                status: "ok".to_string(),
                rs: Some(2.47e-11),
                ng_default: Some(5e-5),
                reference: Some(6.757e-10),
            }),
            contenders: vec![batch, dc, sweep, seq],
            ..Default::default()
        };
        let row = batch_headline_row(&w);
        assert!(
            row[3].starts_with("0.86x (min 0.86x) vs ngspice-dc-sweep-sparse"),
            "{}",
            row[3]
        );
        assert!(row[10].contains("RustSpice best path `rustspice-dc-sweep` 7.36 ms vs ngspice best path `ngspice-dc-sweep-sparse` 18.95 ms: 2.57x"), "{}", row[10]);
        assert!(
            row[10].contains("the batch API row uses 2 thread(s), the ngspice best path 1"),
            "{}",
            row[10]
        );
        // ER-legacy-05 for b6: the oracle error leads, the 6.8e-10 sweep distance is the reference's own error
        assert!(
            row[6].starts_with("2.47e-11 / 5.00e-5 V")
                && row[6].contains("tight reference itself: 6.76e-10"),
            "{}",
            row[6]
        );
        assert_eq!(row[7], "6.76e-10 / 5.00e-5 V (all nodes and corners)");
    }

    #[test]
    fn accuracy_cells_do_not_depend_on_the_timing_repetitions() {
        // --no-timing: nothing completes a repetition, but the correctness run still measured the accuracy
        let untimed = |name: &str, g: f64| {
            let mut c = wc(name, 1.0, Some(g));
            c.stats.n = 0;
            c.stats.reps_planned = 0;
            c.stats.e2e_us = None;
            c
        };
        let w = WorkResult {
            size: 200,
            legacy_size: true,
            contenders: vec![
                untimed("rustspice-adjoint", 2.64e-14),
                untimed("ngspice-sens-sparse", 1e-6),
            ],
            ..Default::default()
        };
        let row = sens_headline_row(&w);
        assert!(row[6].starts_with("2.64e-14 / n/a"), "{}", row[6]);
        assert_eq!(
            (row[1].as_str(), row[2].as_str(), row[3].as_str()),
            ("n/a", "n/a", "n/a")
        );
        assert_eq!(row[8], "n/a (not timed) / n/a", "{row:?}");
        let mut b = untimed("rustspice-batch", 0.0);
        b.corners = Some(TableError {
            rows: 4,
            labels: 3,
            max_abs: 6.76e-10,
            max_rel: 1e-9,
            worst_row: 0,
            worst_label: "v(1)".to_string(),
        });
        let w = WorkResult {
            size: 128,
            legacy_size: true,
            contenders: vec![b],
            ..Default::default()
        };
        let row = batch_headline_row(&w);
        assert!(row[7].starts_with("6.76e-10 / n/a"), "{}", row[7]);
        assert_eq!(row[3], "n/a");
    }

    #[test]
    fn headline_has_one_cell_per_column_and_every_row_states_its_repetitions() {
        let mut r = fake_report();
        r.cases.push(legacy_fake("b2", "tran", 2000));
        let md = headline_section(&r);
        let w = md
            .lines()
            .next()
            .unwrap()
            .replace("\\|", "")
            .matches('|')
            .count();
        assert_eq!(w, 12, "11 columns");
        for l in md.lines() {
            assert_eq!(l.replace("\\|", "").matches('|').count(), w, "{l}");
        }
        assert!(md.contains("e2e speedup, median (min) [headline]"));
        assert!(md.contains("EXCLUDING RustSpice compile"));
        assert!(md.contains("like-for-like analysis speedup"));
        assert!(md.contains("reps n: RustSpice / ngspice"));
        assert!(
            !md.contains("solve-only"),
            "the misleading column name is gone"
        );
    }

    #[test]
    fn no_report_text_calls_the_analysis_ratio_solve_only() {
        let mut r = fake_report();
        r.cases.push(legacy_fake("b2", "tran", 2000));
        let md = render_markdown(&r);
        assert!(
            !md.to_lowercase().contains("solve-only"),
            "{}",
            md.lines()
                .filter(|l| l.to_lowercase().contains("solve-only"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(md.contains("analysis-only") && md.contains("like-for-like"));
        assert!(md.contains("EXCLUDES RustSpice's symbolic analysis"));
    }

    #[test]
    fn narrative_constants_come_from_the_run() {
        // ER-legacy-13: node voltages and attenuation of the legacy ladder from the exact recursion
        // (mpmath, 60 digits: v(2500) = 2.39960633751e-34, v(5001) = 2.12844145451e-68, 5 / v(5001) = 2.34913673073e68)
        let md = changes_section(&fake_report());
        assert!(
            md.contains("v(2500) = 2.40e-34 V and v(5001) = 2.13e-68 V"),
            "{md}"
        );
        assert!(md.contains("attenuates its 5 V input by 2.35e68x"), "{md}");
        for stale in ["4.7e68", "~1e-69", "~1e-34", "published / measured"] {
            assert!(!md.contains(stale), "stale literal {stale}: {md}");
        }
        assert!(
            md.contains("NOT comparable"),
            "cross-run ratios are labelled as such"
        );
        assert!(md.contains("different host and load"));
    }

    #[test]
    fn step_policy_says_what_ngspice_writes_and_what_the_comparison_does() {
        // ER-legacy-14
        assert!(STEP_POLICY.contains("writes ITS OWN internal time points"));
        assert!(STEP_POLICY.contains("interpolates RustSpice, ngspice and the reference linearly onto the nominal `k * tstep` grid"));
        assert!(!STEP_POLICY.contains("output interpolated onto the rawfile rows"));
        let d = case_specs(&["b2".to_string(), "b4".to_string()]);
        assert!(
            d.iter().all(|c| !c.detail.contains("time steps")),
            "{:?}",
            d.iter().map(|c| &c.detail).collect::<Vec<_>>()
        );
        assert!(d.iter().any(|c| c.detail.contains("500 nominal steps")));
    }

    #[test]
    fn git_state_reads_the_given_tree_and_flags_dirty() {
        assert_eq!(git_label(Some("7bb6d36"), Some(false)), " @ 7bb6d36");
        assert_eq!(git_label(Some("7bb6d36"), Some(true)), " @ 7bb6d36-dirty");
        assert_eq!(
            git_label(Some("7bb6d36"), None),
            " @ 7bb6d36 (dirty state unknown)"
        );
        assert_eq!(git_label(None, Some(true)), "");
        // a directory outside any checkout: no hash and no dirty flag (never a made-up value)
        let t = scratch("nogit");
        if git_in(t.path(), &["rev-parse", "--is-inside-work-tree"]).is_none() {
            assert_eq!(git_state(t.path()), (None, None));
        }
        // the binary's own source tree: either unavailable (a copy without .git) or a short hash with a dirty flag
        match git_state(Path::new(env!("CARGO_MANIFEST_DIR"))) {
            (Some(h), d) => {
                assert!(
                    h.len() >= 7 && h.chars().all(|c| c.is_ascii_hexdigit()),
                    "{h}"
                );
                assert!(d.is_some());
            }
            (None, d) => assert!(d.is_none()),
        }
        // the report header carries the flag and warns
        let mut r = fake_report();
        r.rustspice.git = Some("abc1234".to_string());
        r.rustspice.git_dirty = Some(true);
        let md = render_markdown(&r);
        assert!(
            md.contains("RustSpice 0.1.0 @ abc1234-dirty"),
            "{}",
            md.lines().nth(2).unwrap()
        );
        assert!(md.contains("uncommitted changes"));
        assert!(
            !md.contains("/src/rustspice-cli"),
            "no absolute build path in the published text"
        );
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["rustspice"]["source_tree"], "/src/rustspice-cli");
        assert_eq!(j["rustspice"]["git_dirty"], true);
        r.rustspice.git_dirty = Some(false);
        let md = render_markdown(&r);
        assert!(md.contains("RustSpice 0.1.0 @ abc1234,") && !md.contains("uncommitted changes"));
    }

    #[test]
    fn host_threads_report_hardware_and_available_counts() {
        let cpuinfo = "processor\t: 0\nmodel name\t: X\n\nprocessor\t: 1\nmodel name\t: X\n\nprocessor : 2\nsiblings\t: 3\n";
        assert_eq!(count_processors(cpuinfo), Some(3));
        assert_eq!(count_processors("model name : X\n"), None);
        assert_eq!(threads_label(Some(32), 32), "32 hardware threads");
        assert_eq!(
            threads_label(Some(32), 2),
            "32 hardware threads, 2 available to this run (affinity / cgroup limited)"
        );
        assert_eq!(
            threads_label(None, 2),
            "hardware thread count unknown, 2 available to this run"
        );
        let md = render_markdown(&fake_report());
        assert!(
            md.contains("32 hardware threads, 4 available to this run"),
            "{md}"
        );
    }

    #[test]
    fn scaling_uses_the_like_for_like_metric_for_rustspice() {
        let mut cases = Vec::new();
        for (n, t) in [(100usize, 100.0), (1000, 1000.0), (10_000, 10_000.0)] {
            let mut c = fake_case(&format!("b1_ladder_{n}"), n, 1.0);
            // RustSpice analysis ~ n / 10, compile ~ constant 5000 us
            c.contenders = vec![
                cstat_c(RS, t, Some(t / 10.0), Some(5_000.0)),
                cstat(NG_SPARSE, 4.0 * t, Some(t)),
            ];
            cases.push(c);
        }
        let rows = scaling_rows(&cases);
        let fit = |name: &str, metric: &str| {
            rows.iter()
                .find(|r| r.contender == name && r.metric == metric)
                .unwrap()
                .fit
                .exponent
                .unwrap()
        };
        assert!((fit(RS, "analysis") - 1.0).abs() < 1e-9);
        assert!(
            fit(RS, "like_for_like") < 0.9,
            "the constant compile time flattens the curve"
        );
        assert!(
            (fit(NG_SPARSE, "like_for_like") - 1.0).abs() < 1e-9,
            "ngspice uses its own analysis time"
        );
        assert!(rows.iter().all(|r| r.metric != "solve"));
        let md = scaling_section(&rows);
        assert!(
            md.contains("analysis-only (RustSpice without compile)"),
            "{md}"
        );
        assert!(
            md.contains("like-for-like analysis (RustSpice compile + analysis)"),
            "{md}"
        );
    }

    #[test]
    fn verdict_cell_has_a_tie_band_and_names_its_judge() {
        let acc = |rs: f64, ng: f64, oracle: bool| Accuracy {
            status: "ok".to_string(),
            rs_err: Some(rs),
            ng_err: Some(ng),
            oracle_status: oracle.then(|| "ok".to_string()),
            rs_vs_oracle: oracle.then_some(rs),
            ng_vs_oracle: oracle.then_some(ng),
            ..Default::default()
        };
        assert_eq!(
            verdict_cell(&acc(1e-9, 2e-9, true)),
            "yes (independent oracle)"
        );
        // 6.60e-12 vs 6.58e-12 (b1n_ladder_flat_500 of a real run) is a tie, not a "no"
        assert_eq!(
            verdict_cell(&acc(6.60e-12, 6.58e-12, true)),
            "about equal, within 2x (independent oracle)"
        );
        assert_eq!(
            verdict_cell(&acc(9.28e-2, 1.6e-3, false)),
            "no, 58.0x larger (tight reference)"
        );
        assert_eq!(
            verdict_cell(&acc(1e-13, 0.0, false)),
            "n/a (ngspice default run is identical to the reference)"
        );
        assert_eq!(
            verdict_cell(&acc(1e-13, 0.0, true)),
            "no (ngspice default error is 0) (independent oracle)"
        );
        assert_eq!(verdict_cell(&Accuracy::na("x")), "n/a");
        assert_eq!(fmt_agree_v(Some(0.0)), "0 V (identical to the reference)");
        assert_eq!(fmt_agree_v(Some(1.5e-13)), "1.50e-13 V");
    }

    #[test]
    fn sens_plot_vectors_are_counted() {
        let p = plot(
            "Sensitivity Analysis",
            &["v(rsh1:r)", "v(rser1)", "v(rsh1)"],
            vec![vec![0.0], vec![-1.5e-3], vec![4.9e-7]],
        );
        assert_eq!(
            sensitivity_plot(std::slice::from_ref(&p))
                .unwrap()
                .vars
                .len(),
            3
        );
        assert!(sensitivity_plot(&[plot("Operating Point", &["v(1)"], vec![vec![1.0]])]).is_none());
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn dense_reference_error_against_arbitrary_precision_gradients() {
        // ER-legacy-15: exact dv(101)/dR of the 100-stage legacy ladder, mpmath with 50 digits (adjoint of the MNA
        // system), independent of both simulators and of `oracle.rs`. The dense f64 reference and the RustSpice
        // adjoint are both ~1e-13 (scale-normalised) from it; the report says so.
        let exact: [(&str, f64); 12] = [
            ("Rser1", -0.001_290_424_872_782_859_254_154_814_986_66),
            ("Rsh1", 4.031_325_732_235_692_346_318_517_882_92e-8),
            ("Rser2", -0.001_251_361_727_075_962_833_022_321_437_64),
            ("Rsh2", 7.816_535_455_949_973_868_611_958_159_15e-8),
            ("Rser6", -0.001_117_528_279_548_046_568_419_030_335_62),
            ("Rsh6", 2.078_558_290_780_591_783_887_972_051_98e-7),
            ("Rser26", -7.806_212_133_595_849_496_496_885_206_89e-4),
            ("Rsh26", 5.345_357_252_729_345_650_910_219_109_58e-7),
            ("Rser51", -6.545_842_776_846_963_031_961_826_098_05e-4),
            ("Rsh51", 6.580_460_104_290_846_850_226_752_673_57e-7),
            ("Rser76", -5.258_946_545_553_942_416_869_213_262_85e-4),
            ("Rsh100", 1.290_424_872_782_859_254_154_814_986_66e-6),
        ];
        let stages = 100;
        let net = ladder_net(stages, R_SH_LEGACY);
        let reference = net.gradients(stages + 1).unwrap();
        let t = scratch("dense-ref");
        let deck = t.path().join("l.sp");
        fs::write(&deck, ladder_deck(stages, R_SH_LEGACY)).unwrap();
        let out = run_worker_job(&WorkerJob {
            kind: "adjoint".to_string(),
            deck,
            out: t.path().join("o.json"),
            target: Some((stages + 1).to_string()),
            ..Default::default()
        })
        .unwrap();
        let want: Vec<String> = net.res.iter().map(|r| r.0.clone()).collect();
        let rs = order_by_names(&want, &out.names, &out.values).unwrap();
        let scale = exact[0].1.abs(); // Rser1 has the largest gradient magnitude
        let (mut worst_ref, mut worst_rs) = (0.0f64, 0.0f64);
        for (name, e) in exact {
            let i = want.iter().position(|n| n == name).unwrap();
            worst_ref = worst_ref.max((reference[i] - e).abs() / scale);
            worst_rs = worst_rs.max((rs[i] - e).abs() / scale);
        }
        assert!(worst_ref < 3e-13, "dense reference {worst_ref:e}");
        assert!(worst_rs < 3e-13, "RustSpice adjoint {worst_rs:e}");
        // the stated figure is real: the reference is NOT secretly exact (1.2e-13 over all 200 gradients, here the
        // worst of the sampled ones is Rser1)
        assert!(worst_ref > 5e-14, "dense reference {worst_ref:e}");
        assert!(
            DENSE_REF_OWN_ERROR.contains("1.2e-13") && DENSE_REFERENCE_NOTE.contains("1.2e-13")
        );
    }

    // ---- RustSpice library calls against independent references (no ngspice needed)

    fn scratch(tag: &str) -> TmpDir {
        TmpDir::new(&format!("legacy-test-{tag}")).unwrap()
    }

    #[test]
    fn rustspice_adjoint_matches_the_dense_reference() {
        let t = scratch("adj");
        let stages = 12;
        let deck = t.path().join("l.sp");
        fs::write(&deck, ladder_deck(stages, R_SH_LEGACY)).unwrap();
        let job = WorkerJob {
            kind: "adjoint".to_string(),
            deck,
            out: t.path().join("o.json"),
            target: Some((stages + 1).to_string()),
            ..Default::default()
        };
        let out = run_worker_job(&job).unwrap();
        let net = ladder_net(stages, R_SH_LEGACY);
        let want: Vec<String> = net.res.iter().map(|r| r.0.clone()).collect();
        let g = order_by_names(&want, &out.names, &out.values).unwrap();
        let reference = net.gradients(stages + 1).unwrap();
        let e = gradient_error(&g, &reference).unwrap();
        assert!(e.max_over_scale.unwrap() < 1e-9, "{e:?}");
        assert!(out.solve_us > 0.0 && out.parse_us > 0.0 && out.compile_us > 0.0);
        assert_eq!(out.threads, 1, "a sequential worker job runs on one thread");
        // finite differences in the same library agree to FD accuracy
        let fd = run_worker_job(&WorkerJob {
            kind: "fd".to_string(),
            h_rel: Some(FD_H_CENTRAL),
            ..job.clone()
        })
        .unwrap();
        let g2 = order_by_names(&want, &fd.names, &fd.values).unwrap();
        let e2 = gradient_error(&g2, &reference).unwrap();
        assert!(e2.max_over_scale.unwrap() < 1e-6, "{e2:?}");
        assert!(
            e.max_abs < e2.max_abs,
            "the adjoint is more accurate than central FD"
        );
    }

    #[test]
    fn rustspice_batch_matches_the_nonlinear_oracle_and_the_sequential_loop() {
        let t = scratch("batch");
        let deck_text = diode_cascade_deck(6);
        let deck = t.path().join("d.sp");
        fs::write(&deck, &deck_text).unwrap();
        let values = corner_values(5);
        let mk = |kind: &str| WorkerJob {
            kind: kind.to_string(),
            deck: deck.clone(),
            out: t.path().join("o.json"),
            values: values.clone(),
            ..Default::default()
        };
        let batch = run_worker_job(&mk("batch")).unwrap();
        let seq = run_worker_job(&mk("seq")).unwrap();
        assert_eq!(batch.names, seq.names);
        assert_eq!(batch.rows.len(), 5);
        assert_eq!(seq.threads, 1);
        assert_eq!(
            batch.threads,
            worker_threads(),
            "only the batch API uses the pool"
        );
        let e = table_error(&batch.names, &batch.rows, &seq.names, &seq.rows).unwrap();
        assert!(e.max_abs < 1e-9, "batch vs sequential {e:?}");
        // independent dense Newton oracle on a `.dc` deck over exactly these corner values
        let sweep = diode_cascade_deck_with(6, 5.0, &corner_sweep_card(5));
        let l = "v(10)";
        let o = oracle::evaluate(&sweep, "dc_nonlinear", l, &values).unwrap();
        let j = batch.names.iter().position(|n| n == l).unwrap();
        for (c, row) in batch.rows.iter().enumerate() {
            assert!(
                (row[j] - o[c]).abs() < 1e-8,
                "corner {c}: {} vs {}",
                row[j],
                o[c]
            );
        }
    }

    // ---- ngspice-dependent checks (skipped with a message when ngspice is not installed)

    fn ngspice_available() -> bool {
        let ok = ngspice::version("ngspice").is_some();
        if !ok {
            eprintln!(
                "ngspice not installed: ngspice-dependent legacy checks are UNVERIFIED (skipped)"
            );
        }
        ok
    }

    fn test_ctx(tag: &str) -> Ctx {
        let lim = Limits {
            timeout: Duration::from_secs(60),
            max_rss_kib: Some(CHILD_RSS_CAP_KIB),
            pin_cpu: None,
        };
        Ctx {
            exe: PathBuf::from("/nonexistent/rustspice"),
            ng: "ngspice".to_string(),
            ng_ok: true,
            ng_version: None,
            tmp: scratch(tag),
            reps: 3,
            timing: false,
            lim: lim.clone(),
            lim_free: lim,
            threads: 2,
            floors: Floors::default(),
        }
    }

    #[test]
    fn ngspice_sens_and_fd_agree_with_the_dense_reference() {
        if !ngspice_available() {
            return;
        }
        let ctx = test_ctx("ngsens");
        let stages = 5;
        let net = ladder_net(stages, R_SH_LEGACY);
        let names: Vec<String> = net.res.iter().map(|r| r.0.clone()).collect();
        let reference = net.gradients(stages + 1).unwrap();
        // .sens
        let dir = ctx.tmp.path();
        let sens = ladder_deck_with(
            stages,
            R_SH_LEGACY,
            &format!(".sens v({})", stages + 1),
            None,
        );
        fs::write(dir.join("s.sp"), sens).unwrap();
        let s = ng_run_sample(
            &ctx,
            &dir.join("s.sp"),
            &dir.join("s.raw"),
            NgSolver::Sparse,
            &ctx.lim,
            true,
        )
        .unwrap();
        let Some(Output::Plots(p)) = s.output else {
            panic!("no plots")
        };
        let g = sens_gradients(&p, &names).unwrap();
        let e = gradient_error(&g, &reference).unwrap();
        assert!(e.max_over_scale.unwrap() < 1e-5, ".sens error {e:?}");
        // alter loop
        let perts = fd_perturbations(&net, FD_H_CENTRAL);
        let ctl = fd_control_deck(&net, R_SH_LEGACY, stages + 1, &perts);
        fs::write(dir.join("c.sp"), ctl).unwrap();
        let mut cmd = Command::new("ngspice");
        cmd.arg("-b").arg(dir.join("c.sp"));
        let child = run_guarded(&mut cmd, &ctx.lim).unwrap();
        let v = parse_prints(&child.stdout, &format!("v({})", stages + 1));
        assert_eq!(v.len(), 2 * perts.len(), "{}", child.stdout);
        let g: Vec<f64> = perts
            .iter()
            .enumerate()
            .map(|(i, (p, m))| (v[2 * i] - v[2 * i + 1]) / (p - m))
            .collect();
        let e = gradient_error(&g, &reference).unwrap();
        assert!(e.max_over_scale.unwrap() < 1e-6, "FD loop error {e:?}");
    }

    #[test]
    fn ngspice_ladder_op_matches_the_exact_recursion() {
        if !ngspice_available() {
            return;
        }
        let ctx = test_ctx("ngop");
        let n = 40;
        let dir = ctx.tmp.path();
        fs::write(dir.join("l.sp"), ladder_deck(n, R_SH_LEGACY)).unwrap();
        fs::write(
            dir.join("lk.sp"),
            meta::inject_options(&ladder_deck(n, R_SH_LEGACY), "klu"),
        )
        .unwrap();
        let s = ng_run_sample(
            &ctx,
            &dir.join("lk.sp"),
            &dir.join("l.raw"),
            NgSolver::Klu,
            &ctx.lim,
            true,
        )
        .unwrap();
        assert!(s.analysis_us.is_some() && s.rss_kib.is_some());
        let Some(Output::Plots(p)) = s.output else {
            panic!("no plots")
        };
        let truth = ladder_voltages(n, R_SER, R_SH_LEGACY, 5.0);
        let plot = crate::bench::rawfile::last_plot(&p, PlotKind::Op).unwrap();
        assert!(ladder_plot_error(plot, &truth).unwrap() < 1e-9);
    }
}
