//! `rustspice bench-batch`: batched DC operating points of the b6 diode cascade and the b4 CMOS inverter chain at
//! many corners, on the CPU solver's batch API (`Simulator::run_batch_dc_op`), the batch kernel's rayon `cpu_ref`
//! and a CUDA GPU (`Simulator::run_batch_dc_op_with`).
//!
//! Every time is end to end: from the list of `BatchCircuitInput`s to one `BatchCircuitOutput` per corner, so it
//! includes plan building, parameter mapping, host-device transfers, CPU re-solves of corners the kernel leaves and
//! the output vectors. The GPU's one-off initialisation (driver, context, PTX JIT) is measured once per process and
//! reported on its own and in the single-shot column.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use clap::Args;
use rustspice_backend_cpu::{BatchCircuitInput, BatchCircuitOutput};
use rustspice_backend_cuda_simt::batch::CudaBatchSolver;
use rustspice_solver::{BatchDcEngine, Simulator};

use super::legacy::{cmos_chain_deck, cpu_model, diode_cascade_deck_with, git_label, git_state};
use super::rustsim::loadavg;

#[derive(Args, Debug, Clone)]
pub struct BatchArgs {
    /// Corner counts of the supply workload (comma separated)
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "128,1024,16384,131072,1048576"
    )]
    pub corners: Vec<usize>,
    /// Corner counts of the Monte Carlo workload (every resistor and every diode IS varies)
    #[arg(long, value_delimiter = ',', default_value = "128,1024,16384")]
    pub mc_corners: Vec<usize>,
    /// Diodes of the cascade (b6 uses 32)
    #[arg(long, default_value_t = 32)]
    pub diodes: usize,
    /// Corner counts of the CMOS inverter chain's supply and input corners
    #[arg(long, value_delimiter = ',', default_value = "128,1024,16384,131072")]
    pub mos_corners: Vec<usize>,
    /// Corner counts of the CMOS inverter chain's Monte Carlo workload (VTO and KP of every MOSFET vary)
    #[arg(long, value_delimiter = ',', default_value = "128,1024,16384")]
    pub mos_mc_corners: Vec<usize>,
    /// Stages of the CMOS inverter chain (b4 uses 100)
    #[arg(long, default_value_t = 100)]
    pub stages: usize,
    /// Repetitions per measurement (the median is reported; at most 3 above 100k corners)
    #[arg(long, default_value_t = 5)]
    pub reps: usize,
    /// Largest corner count run through the CPU solver's batch API (it clones the topology for every corner)
    #[arg(long, default_value_t = 131072)]
    pub cpu_api_max: usize,
    /// Register cap per GPU thread for the PTX JIT (default: the solver's)
    #[arg(long)]
    pub gpu_max_registers: Option<u32>,
    /// Device memory per kernel launch in MiB (default: half the free memory, at most 2 GiB)
    #[arg(long)]
    pub gpu_launch_mib: Option<usize>,
    /// Write the Markdown report here (default: stdout)
    #[arg(long)]
    pub output: Option<PathBuf>,
}

/// Supply corners spread over the b6 range (1.5 V to 5.31 V; 128 corners are exactly b6's).
fn supply_inputs(n: usize) -> Vec<BatchCircuitInput> {
    (0..n)
        .map(|c| {
            let t = if n > 1 {
                c as f64 / (n - 1) as f64
            } else {
                0.0
            };
            BatchCircuitInput {
                overrides: vec![("V1".into(), "dc".into(), 1.5 + 3.81 * t)],
            }
        })
        .collect()
}

/// Monte Carlo corners: supply 3.3 V +- 5 %, every resistor with a 1 % Gaussian spread, every diode IS log-normal
/// with a 10 % spread (deterministic seed).
fn monte_carlo_inputs(n: usize, diodes: usize) -> Vec<BatchCircuitInput> {
    let mut gauss = gaussian(0x9e37_79b9_7f4a_7c15);
    (0..n)
        .map(|_| {
            let mut o = Vec::with_capacity(3 * diodes + 1);
            o.push(("V1".into(), "dc".into(), 3.3 * (1.0 + 0.05 * gauss())));
            for k in 1..=diodes {
                o.push((format!("R{k}"), "r".into(), 1000.0 * (1.0 + 0.01 * gauss())));
                o.push((
                    format!("Rlink{k}"),
                    "r".into(),
                    10000.0 * (1.0 + 0.01 * gauss()),
                ));
                o.push((format!("D{k}"), "is".into(), 1e-14 * (0.1 * gauss()).exp()));
            }
            BatchCircuitInput { overrides: o }
        })
        .collect()
}

/// A deterministic standard normal generator: Box-Muller over a 64-bit LCG seeded with `seed`.
fn gaussian(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    let mut uniform = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    move || {
        let (u1, u2) = (uniform(), uniform());
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// Supply and input corners of the inverter chain: VDD from 1.8 V to 3.6 V, the input from 0 to VDD (a grid of
/// 64 input levels per supply; fewer than 64 corners sweep the input at 3.3 V).
fn chain_inputs(n: usize) -> Vec<BatchCircuitInput> {
    let levels = n.min(64);
    let supplies = n.div_ceil(levels);
    (0..n)
        .map(|c| {
            let vdd = if supplies > 1 {
                1.8 + 1.8 * (c / levels) as f64 / (supplies - 1) as f64
            } else {
                3.3
            };
            let vin = vdd * (c % levels) as f64 / (levels.max(2) - 1) as f64;
            BatchCircuitInput {
                overrides: vec![
                    ("VDD".into(), "dc".into(), vdd),
                    ("VIN".into(), "dc".into(), vin),
                ],
            }
        })
        .collect()
}

/// Monte Carlo corners of the inverter chain: VDD 3.3 V +- 5 %, the input at a quarter of VDD (the first stages in
/// their transition region), and for every MOSFET `VTO` with a 15 mV and `KP` with a 3 % Gaussian spread.
fn chain_monte_carlo_inputs(n: usize, stages: usize) -> Vec<BatchCircuitInput> {
    let mut gauss = gaussian(0x2545_f491_4f6c_dd1d);
    (0..n)
        .map(|_| {
            let vdd = 3.3 * (1.0 + 0.05 * gauss());
            let mut o = Vec::with_capacity(4 * stages + 2);
            o.push(("VDD".into(), "dc".into(), vdd));
            o.push(("VIN".into(), "dc".into(), 0.25 * vdd));
            for k in 1..=stages {
                o.push((format!("MN{k}"), "vto".into(), 0.7 + 0.015 * gauss()));
                o.push((
                    format!("MN{k}"),
                    "kp".into(),
                    120e-6 * (1.0 + 0.03 * gauss()),
                ));
                o.push((format!("MP{k}"), "vto".into(), -0.7 + 0.015 * gauss()));
                o.push((
                    format!("MP{k}"),
                    "kp".into(),
                    60e-6 * (1.0 + 0.03 * gauss()),
                ));
            }
            BatchCircuitInput { overrides: o }
        })
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Largest `|a - b| / (1 + max(|a|, |b|))` over all corners and unknowns.
fn max_rel_diff(a: &[BatchCircuitOutput], b: &[BatchCircuitOutput]) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| x.solution.iter().zip(&y.solution))
        .map(|(&p, &q)| (p - q).abs() / (1.0 + p.abs().max(q.abs())))
        .fold(0.0, f64::max)
}

struct Row {
    corners: usize,
    cpu_api_ms: Option<f64>,
    cpu_ref_ms: f64,
    cuda_ms: Option<f64>,
    diff_ref_api: Option<f64>,
    diff_cuda_api: Option<f64>,
    diff_cuda_ref: Option<f64>,
    fallback_ref: usize,
    fallback_cuda: Option<usize>,
    /// `(plan + parameters, kernel solve)` of the CUDA run, ms
    cuda_split: Option<(f64, f64)>,
    /// Mean Newton iterations per corner (kernel or CPU re-solve)
    iterations: f64,
}

fn timed<T>(reps: usize, mut f: impl FnMut() -> T) -> (f64, T) {
    let mut times = Vec::with_capacity(reps);
    let mut last = None;
    for _ in 0..reps.max(1) {
        let t0 = Instant::now();
        let r = f();
        times.push(t0.elapsed().as_secs_f64() * 1e3);
        last = Some(r);
    }
    (median(times), last.unwrap())
}

fn run_workload(
    sim: &Simulator,
    gpu: Option<&CudaBatchSolver>,
    args: &BatchArgs,
    name: &str,
    counts: &[usize],
    make: &dyn Fn(usize) -> Vec<BatchCircuitInput>,
) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for &n in counts {
        let reps = if n > 100_000 {
            args.reps.min(3)
        } else {
            args.reps
        };
        eprintln!("bench-batch: {name}, {n} corners, {reps} reps");
        let inputs = make(n);
        let cpu_api = if n <= args.cpu_api_max {
            let (ms, out) = timed(reps, || sim.run_batch_dc_op(&inputs));
            Some((ms, out.map_err(|e| e.to_string())?))
        } else {
            None
        };
        let (cpu_ref_ms, rref) = timed(reps, || {
            sim.run_batch_dc_op_with(&inputs, BatchDcEngine::CpuRef)
        });
        let rref = rref.map_err(|e| e.to_string())?;
        if let Some(why) = &rref.unsupported {
            return Err(format!(
                "the batch kernel does not support the {name} workload: {why}"
            ));
        }
        let cuda = match gpu {
            Some(g) => {
                let (ms, r) = timed(reps, || {
                    sim.run_batch_dc_op_with(&inputs, BatchDcEngine::Cuda(g))
                });
                let r = r.map_err(|e| e.to_string())?;
                // where the time goes: host plan + parameter mapping, then the solve (transfers + kernel)
                let t0 = Instant::now();
                let (plan, params) = sim.batch_dc_plan(&inputs)?;
                let plan_ms = t0.elapsed().as_secs_f64() * 1e3;
                let t1 = Instant::now();
                g.solve(&plan, &params, n, None)
                    .map_err(|e| e.to_string())?;
                let solve_ms = t1.elapsed().as_secs_f64() * 1e3;
                Some((ms, r, (plan_ms, solve_ms)))
            }
            None => None,
        };
        rows.push(Row {
            corners: n,
            cpu_api_ms: cpu_api.as_ref().map(|c| c.0),
            cpu_ref_ms,
            cuda_ms: cuda.as_ref().map(|c| c.0),
            diff_ref_api: cpu_api.as_ref().map(|c| max_rel_diff(&rref.outputs, &c.1)),
            diff_cuda_api: match (&cuda, &cpu_api) {
                (Some(g), Some(c)) => Some(max_rel_diff(&g.1.outputs, &c.1)),
                _ => None,
            },
            diff_cuda_ref: cuda
                .as_ref()
                .map(|g| max_rel_diff(&g.1.outputs, &rref.outputs)),
            fallback_ref: rref.cpu_fallback,
            fallback_cuda: cuda.as_ref().map(|g| g.1.cpu_fallback),
            cuda_split: cuda.as_ref().map(|g| g.2),
            iterations: rref
                .outputs
                .iter()
                .map(|o| o.iterations as f64)
                .sum::<f64>()
                / n.max(1) as f64,
        });
    }
    Ok(rows)
}

fn ms(v: Option<f64>) -> String {
    match v {
        Some(t) if t >= 100.0 => format!("{t:.0} ms"),
        Some(t) if t >= 1.0 => format!("{t:.1} ms"),
        Some(t) => format!("{t:.2} ms"),
        None => "not run".into(),
    }
}

fn sci(v: Option<f64>) -> String {
    v.map_or("n/a".into(), |d| format!("{d:.1e}"))
}

fn table(s: &mut String, rows: &[Row], init_ms: Option<f64>) {
    let _ = writeln!(
        s,
        "| corners | CPU batch API | cpu_ref (rayon) | CUDA | CUDA incl. init | CUDA vs fastest CPU | corners/s (CUDA) | max rel. diff. cpu_ref / CUDA vs CPU API | CUDA vs cpu_ref | CPU re-solves (cpu_ref / CUDA) |"
    );
    let _ = writeln!(s, "|---:|---:|---:|---:|---:|---:|---:|---|---:|---|");
    for r in rows {
        let best_cpu = r.cpu_api_ms.map_or(r.cpu_ref_ms, |a| a.min(r.cpu_ref_ms));
        let speed = r
            .cuda_ms
            .map_or("n/a".into(), |g| format!("{:.2}x", best_cpu / g));
        let rate = r.cuda_ms.map_or("n/a".into(), |g| {
            format!("{:.2e}", r.corners as f64 / (g * 1e-3))
        });
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} | {} / {} | {} | {} / {} |",
            r.corners,
            ms(r.cpu_api_ms),
            ms(Some(r.cpu_ref_ms)),
            ms(r.cuda_ms),
            ms(r.cuda_ms.zip(init_ms).map(|(a, b)| a + b)),
            speed,
            rate,
            sci(r.diff_ref_api),
            sci(r.diff_cuda_api),
            sci(r.diff_cuda_ref),
            r.fallback_ref,
            r.fallback_cuda.map_or("n/a".into(), |f| f.to_string()),
        );
    }
}

fn split(s: &mut String, rows: &[Row]) {
    if let Some(r) = rows.iter().rev().find(|r| r.cuda_split.is_some()) {
        let (plan, solve) = r.cuda_split.unwrap();
        let total = r.cuda_ms.unwrap();
        let _ = writeln!(
            s,
            "\nAt {} corners (mean {:.2} Newton iterations per corner) the CUDA run splits into {} of host work before the GPU (circuit state at x = 0, LU plan, parameter mapping), {} in `CudaBatchSolver::solve` (transfers and kernel; one sample, outside the median) and about {} building the output vectors.",
            r.corners,
            r.iterations,
            ms(Some(plan)),
            ms(Some(solve)),
            ms(Some((total - plan - solve).max(0.0)))
        );
    }
}

/// Runs the benchmark; returns the process exit code.
pub fn run(args: BatchArgs) -> i32 {
    match run_inner(&args) {
        Ok(report) => {
            if let Some(p) = &args.output {
                if let Err(e) = std::fs::write(p, &report) {
                    eprintln!("bench-batch: cannot write {}: {e}", p.display());
                    return 2;
                }
                eprintln!("bench-batch: wrote {}", p.display());
            } else {
                print!("{report}");
            }
            0
        }
        Err(e) => {
            eprintln!("bench-batch: {e}");
            1
        }
    }
}

fn run_inner(args: &BatchArgs) -> Result<String, String> {
    let load_start = loadavg();
    let deck = diode_cascade_deck_with(args.diodes, 5.0, ".op");
    let sim = Simulator::from_spice(&deck).map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let gpu = match args.gpu_max_registers {
        Some(r) => CudaBatchSolver::with_max_registers(0, Some(r)),
        None => CudaBatchSolver::new(0),
    }
    .map(|mut g| {
        g.launch_budget = args.gpu_launch_mib.map(|m| m << 20);
        g
    });
    let init_ms = t0.elapsed().as_secs_f64() * 1e3;
    let (gpu, gpu_line, init) = match gpu {
        Ok(g) => {
            let line = format!(
                "{} (CUDA driver {}); initialisation in this process (driver, context, PTX JIT): {}",
                g.device_name(),
                g.driver_version(),
                ms(Some(init_ms))
            );
            (Some(g), line, Some(init_ms))
        }
        Err(e) => (None, format!("none ({e}): CUDA columns not run"), None),
    };
    let (plan, _) = sim.batch_dc_plan(&supply_inputs(1))?;
    let chain_deck = cmos_chain_deck(args.stages);
    let chain = Simulator::from_spice(&chain_deck).map_err(|e| e.to_string())?;
    let (chain_plan, _) = chain.batch_dc_plan(&chain_inputs(1))?;
    let supply = run_workload(
        &sim,
        gpu.as_ref(),
        args,
        "supply",
        &args.corners,
        &supply_inputs,
    )?;
    let mc = run_workload(
        &sim,
        gpu.as_ref(),
        args,
        "monte-carlo",
        &args.mc_corners,
        &|n| monte_carlo_inputs(n, args.diodes),
    )?;
    let chain_grid = run_workload(
        &chain,
        gpu.as_ref(),
        args,
        "inverter chain",
        &args.mos_corners,
        &chain_inputs,
    )?;
    let chain_mc = run_workload(
        &chain,
        gpu.as_ref(),
        args,
        "inverter chain monte-carlo",
        &args.mos_mc_corners,
        &|n| chain_monte_carlo_inputs(n, args.stages),
    )?;

    let load_end = loadavg();
    let (head, dirty) = git_state(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let mut s = String::new();
    let _ = writeln!(s, "# Batched DC operating points: CPU vs CUDA\n");
    let _ = writeln!(
        s,
        "Generated by `rustspice bench-batch`{}. Circuits: the {}-diode cascade of b6 (MNA dimension {}, {} workspace words per corner) and the {}-stage CMOS inverter chain of b4 ({} level-1 MOSFETs with their bulk junctions, MNA dimension {}, {} workspace words per corner).\n",
        git_label(head.as_deref(), dirty),
        args.diodes,
        plan.dim(),
        plan.workspace_slots(),
        args.stages,
        2 * args.stages,
        chain_plan.dim(),
        chain_plan.workspace_slots()
    );
    let load = |l: Option<f64>| l.map_or("n/a".to_string(), |x| format!("{x:.1}"));
    let _ = writeln!(
        s,
        "- CPU: {} ({threads} rayon threads); 1-minute load average {} at start, {} at end (the end value includes this benchmark's own threads)",
        cpu_model(),
        load(load_start),
        load(load_end)
    );
    let _ = writeln!(s, "- GPU: {gpu_line}");
    let _ = writeln!(
        s,
        "- Times are end to end, median of {} repetitions (3 above 100k corners): from the `BatchCircuitInput` list to one solution vector per corner, including plan building, parameter mapping, transfers and CPU re-solves.",
        args.reps
    );
    let _ = writeln!(
        s,
        "- CPU batch API: `Simulator::run_batch_dc_op` (rayon, one topology clone and one full CPU solve per corner). cpu_ref: the batch kernel's CPU build (rayon). CUDA: the same kernel source compiled to PTX by cuda-oxide, one corner per thread, FP64.\n- Differences are `max |a - b| / (1 + max(|a|, |b|))` over all corners and unknowns. Every corner the kernel does not converge is re-solved by the CPU solver (counted in the last column).\n"
    );
    let _ = writeln!(s, "## Supply corners (V1 from 1.5 V to 5.31 V)\n");
    table(&mut s, &supply, init);
    split(&mut s, &supply);
    let _ = writeln!(
        s,
        "\n## Monte Carlo corners (V1, every resistor and every diode IS varied: {} parameters per corner)\n",
        3 * args.diodes + 1
    );
    table(&mut s, &mc, init);
    split(&mut s, &mc);
    let _ = writeln!(
        s,
        "\n## CMOS inverter chain: supply and input corners (VDD from 1.8 V to 3.6 V, the input from 0 to VDD)\n"
    );
    table(&mut s, &chain_grid, init);
    split(&mut s, &chain_grid);
    let _ = writeln!(
        s,
        "\n## CMOS inverter chain: Monte Carlo corners (VDD, and VTO and KP of every MOSFET: {} parameters per corner)\n",
        4 * args.stages + 2
    );
    table(&mut s, &chain_mc, init);
    split(&mut s, &chain_mc);
    Ok(s)
}
