//! Command-line interface (`rustspice`) and automated `ngspice` head-to-head verification
//! and performance benchmark runner.

mod bench;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};
use rustspice_backend_cpu::CpuSimdBackend;
use rustspice_backend_cuda_simt::CudaSimtBackend;
use rustspice_backend_cuda_tile::{CudaTileBackend, TileShape};
use rustspice_backend_metal::MetalUmaBackend;
use rustspice_backend_wasm::WasmWebGpuBackend;
use rustspice_core::{AnalysisCmd, ComputeBackend, Diagnostic, ResourceLimits, Severity};
use rustspice_ir::{
    build_diode_ir, build_mosfet_ekv_ir, build_mosfet_level1_ir, emit_cuda_ptx_kernel,
    emit_cuda_tile_kernel, emit_msl_df64_kernel, emit_rust_scalar_fn, emit_wgsl_df64_kernel,
};
use rustspice_solver::{ParameterGradSpec, Simulator};

#[derive(Parser, Debug)]
#[command(
    name = "rustspice",
    version,
    about = "Next-Generation Heterogeneous Differentiable SPICE Circuit Simulator"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a SPICE netlist simulation (`.op`, `.dc`, `.tran`)
    Run {
        /// Path to the input SPICE `.sp` / `.cir` netlist file
        netlist: PathBuf,
        /// Hardware execution backend (`cpu`, `metal`, `cuda-tile`, `cuda-simt`, `wasm`)
        #[arg(long, default_value = "cpu")]
        backend: String,
        /// Write an ngspice-compatible binary rawfile (one plot per analysis, in execution order)
        #[arg(long)]
        raw: Option<PathBuf>,
        /// Write a JSON timing/status summary (unavailable numbers are null)
        #[arg(long)]
        stats: Option<PathBuf>,
        /// Refuse the deck (exit 1) when the reader had to ignore, approximate or re-interpret anything
        #[arg(long)]
        strict: bool,
        /// Maximum stored `.dc` / `.tran` points per analysis [default: 50000000]
        #[arg(long)]
        max_points: Option<u64>,
        /// Maximum MiB held by the stored result of one analysis [default: 1024]
        #[arg(long)]
        max_result_mib: Option<u64>,
        /// Maximum `.subckt` instantiation depth [default: 1000]
        #[arg(long)]
        max_subckt_depth: Option<usize>,
        /// Maximum cards visited while flattening subcircuits [default: 2000000]
        #[arg(long)]
        max_expanded_cards: Option<usize>,
        /// Maximum MiB of flattened netlist text [default: 256]
        #[arg(long)]
        max_expanded_mib: Option<usize>,
    },
    /// Compute Discrete Adjoint parameter sensitivities at the DC operating point
    Adjoint {
        /// Path to the input SPICE netlist file
        netlist: PathBuf,
        /// Target node name whose voltage sensitivity dV(node)/d(theta) is computed
        #[arg(long)]
        target_node: String,
    },
    /// Emit auto-differentiated CSE-optimized device kernel code for a target backend
    Codegen {
        /// Device model (`diode`, `mosfet-l1`, `mosfet-ekv`)
        #[arg(long, default_value = "mosfet-l1")]
        model: String,
        /// Target codegen language (`msl`, `wgsl`, `cuda-tile`, `cuda-ptx`, `rust`)
        #[arg(long, default_value = "msl")]
        target: String,
    },
    /// Run the differential capability / accuracy / robustness / performance suite against local `ngspice`
    BenchSuite(bench::SuiteArgs),
    /// Run the six legacy head-to-head benchmarks against local `ngspice` (interleaved subprocess timing, tight references)
    BenchNgspice(bench::legacy::LegacyArgs),
}

fn select_backend(name: &str) -> Result<Arc<dyn ComputeBackend>, String> {
    match name.to_ascii_lowercase().as_str() {
        "cpu" | "cpu-simd" => Ok(Arc::new(CpuSimdBackend::new())),
        "metal" => Ok(Arc::new(MetalUmaBackend::new())),
        "cuda-tile" => Ok(Arc::new(CudaTileBackend::new(TileShape::default()))),
        "cuda-simt" => Ok(Arc::new(CudaSimtBackend::new())),
        "wasm" => Ok(Arc::new(WasmWebGpuBackend::default())),
        other => Err(format!(
            "unknown backend `{other}` (expected one of: cpu, metal, cuda-tile, cuda-simt, wasm)"
        )),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            netlist,
            backend,
            raw,
            stats,
            strict,
            max_points,
            max_result_mib,
            max_subckt_depth,
            max_expanded_cards,
            max_expanded_mib,
        } => {
            let mut limits = ResourceLimits::default();
            if let Some(v) = max_points {
                limits.max_points = v;
            }
            if let Some(v) = max_result_mib {
                limits.max_result_bytes = v.saturating_mul(1 << 20);
            }
            if let Some(v) = max_subckt_depth {
                limits.max_subckt_depth = v;
            }
            if let Some(v) = max_expanded_cards {
                limits.max_expanded_cards = v;
            }
            if let Some(v) = max_expanded_mib {
                limits.max_expanded_bytes = v.saturating_mul(1 << 20);
            }
            if let Err(e) = run_deck(
                &netlist,
                &backend,
                raw.as_deref(),
                stats.as_deref(),
                strict,
                &limits,
            ) {
                if e.downcast_ref::<RawWriteError>().is_some() {
                    // a harness-side I/O failure (full or read-only scratch disk) is not a simulator failure:
                    // the dedicated exit code lets `bench-suite` classify the case `unverified`, not `loud_error`
                    eprintln!("Error: {e}");
                    std::process::exit(bench::rustsim::HARNESS_IO_EXIT);
                }
                return Err(e);
            }
        }
        Commands::Adjoint {
            netlist,
            target_node,
        } => {
            let src = fs::read_to_string(&netlist)?;
            let mut sim = Simulator::from_spice(&src)?;
            report_diagnostics(&sim.netlist.diagnostics);
            let mut specs: Vec<ParameterGradSpec> = Vec::new();
            for rname in &sim.topology.soa.res_names {
                specs.push(ParameterGradSpec::new(rname.clone(), "r"));
            }
            for dname in &sim.topology.soa.diode_names {
                specs.push(ParameterGradSpec::new(dname.clone(), "is"));
            }
            for mname in &sim.topology.soa.mos_names {
                specs.push(ParameterGradSpec::new(mname.clone(), "w"));
                specs.push(ParameterGradSpec::new(mname.clone(), "vto"));
            }
            let t0 = Instant::now();
            let (_op, adj) = sim.run_dc_adjoint(&target_node, &specs)?;
            let adj_us = t0.elapsed().as_secs_f64() * 1e6;
            println!(
                "[Discrete Adjoint] Computed {} parameter sensitivities for V({target_node}) in {:.2} us:",
                adj.gradients.len(),
                adj_us
            );
            for (spec, &grad) in specs.iter().zip(adj.gradients.iter()) {
                println!(
                    "  dV({target_node})/d({}.{}) = {:.9e}",
                    spec.device_name, spec.param_name, grad
                );
            }
        }
        Commands::Codegen { model, target } => {
            let ir = match model.as_str() {
                "diode" => build_diode_ir(),
                "mosfet-l1" => build_mosfet_level1_ir(),
                "mosfet-ekv" => build_mosfet_ekv_ir(),
                other => {
                    return Err(format!(
                        "unknown device model `{other}` (expected one of: diode, mosfet-l1, mosfet-ekv)"
                    )
                    .into());
                }
            };
            let code = match target.as_str() {
                "msl" => emit_msl_df64_kernel(&ir),
                "wgsl" => emit_wgsl_df64_kernel(&ir),
                "cuda-tile" => emit_cuda_tile_kernel(&ir, 128),
                "cuda-ptx" => emit_cuda_ptx_kernel(&ir),
                "rust" => emit_rust_scalar_fn(&ir),
                other => {
                    return Err(format!(
                        "unknown codegen target `{other}` (expected one of: msl, wgsl, cuda-tile, cuda-ptx, rust)"
                    )
                    .into());
                }
            };
            println!("{code}");
        }
        Commands::BenchSuite(args) => {
            std::process::exit(bench::run_suite(args));
        }
        Commands::BenchNgspice(args) => {
            std::process::exit(bench::legacy::run(args));
        }
    }
    Ok(())
}

/// One analysis result waiting to be written to the rawfile.
struct RawPlotOut {
    plotname: &'static str,
    vars: Vec<bench::rawfile::Var>,
    /// Sweep / time axis (becomes variable 0); `None` for `.op`.
    axis: Option<Vec<f64>>,
    rows: Vec<Vec<f64>>,
}

/// `v(node)` / `i(vsrc)` / `i(lname)` labels of the MNA unknowns, lower-case like ngspice.
fn mna_labels(sim: &Simulator) -> Vec<String> {
    let soa = &sim.topology.soa;
    let mut l: Vec<String> = vec![String::new(); soa.mna_dim];
    for (i, n) in sim.netlist.node_names.iter().enumerate().skip(1) {
        l[i - 1] = format!("v({})", n.to_ascii_lowercase());
    }
    for (k, name) in soa.vsrc_names.iter().enumerate() {
        l[soa.vsrc_branch_idx[k]] = format!("i({})", name.to_ascii_lowercase());
    }
    for (k, name) in soa.ind_names.iter().enumerate() {
        l[soa.ind_branch_idx[k]] = format!("i({})", name.to_ascii_lowercase());
    }
    for (i, s) in l.iter_mut().enumerate() {
        if s.is_empty() {
            *s = format!("x({i})");
        }
    }
    l
}

fn raw_vars(labels: &[String]) -> Vec<bench::rawfile::Var> {
    labels
        .iter()
        .map(|n| bench::rawfile::Var {
            name: n.clone(),
            kind: if n.starts_with("i(") {
                "current"
            } else {
                "voltage"
            }
            .to_string(),
        })
        .collect()
}

/// Peak resident set (`VmHWM`) of this process in KiB, if `/proc` is available.
fn peak_rss_kib() -> Option<u64> {
    let s = fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

#[derive(Default, serde::Serialize)]
struct AnalysisStat {
    analysis: &'static str,
    analysis_us: Option<f64>,
    points: Option<u64>,
}

#[derive(Default, serde::Serialize)]
struct RunStats {
    parse_us: Option<f64>,
    compile_us: Option<f64>,
    solver_kind: Option<String>,
    /// Not separable from `compile_us` with the current `Simulator` facade.
    solver_compile_us: Option<f64>,
    analysis: Option<String>,
    analysis_us: Option<f64>,
    write_us: Option<f64>,
    peak_rss_kib: Option<u64>,
    points: Option<u64>,
    status: Option<String>,
    analyses: Vec<AnalysisStat>,
}

/// Prints every reader diagnostic on stderr (`warning[code]: line N: ...`) and returns the number of warnings.
/// Diagnostics never change the exit code unless `--strict` is given.
fn report_diagnostics(diags: &[Diagnostic]) -> usize {
    let mut warnings = 0;
    for d in diags {
        eprintln!("{}", d.render());
        if d.severity == Severity::Warning {
            warnings += 1;
        }
    }
    if warnings > 0 {
        eprintln!(
            "rustspice: {warnings} reader warning(s): parts of this deck were ignored, approximated or re-interpreted, so the results may differ from SPICE (--strict refuses such decks)"
        );
    }
    warnings
}

/// `rustspice run`: unchanged stdout; optionally a rawfile (written only after every analysis succeeded) and a
/// stats JSON (also written on failure, with `status = "error: ..."`).
fn run_deck(
    netlist: &std::path::Path,
    backend: &str,
    raw: Option<&std::path::Path>,
    stats_path: Option<&std::path::Path>,
    strict: bool,
    limits: &ResourceLimits,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(r) = raw {
        // never leave a stale or partial rawfile behind a failed run
        let _ = fs::remove_file(r);
    }
    let mut stats = RunStats::default();
    let result = run_deck_inner(netlist, backend, raw, &mut stats, strict, limits);
    stats.peak_rss_kib = peak_rss_kib();
    stats.status = Some(match &result {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("error: {e}"),
    });
    if let Some(sp) = stats_path {
        if let Ok(text) = serde_json::to_string_pretty(&stats) {
            if let Err(e) = fs::write(sp, text) {
                eprintln!("warning: cannot write stats file {}: {e}", sp.display());
            }
        }
    }
    result
}

fn run_deck_inner(
    netlist: &std::path::Path,
    backend: &str,
    raw: Option<&std::path::Path>,
    stats: &mut RunStats,
    strict: bool,
    limits: &ResourceLimits,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = fs::read_to_string(netlist)?;
    let backend_arc = select_backend(backend)?;
    let t0 = Instant::now();
    let parsed = rustspice_core::parse_netlist_with_limits(&src, limits)?;
    stats.parse_us = Some(t0.elapsed().as_secs_f64() * 1e6);
    let n_warnings = report_diagnostics(&parsed.diagnostics);
    for r in &parsed.resistors {
        if rustspice_core::soa::resistance_is_floored(r.resistance) {
            eprintln!(
                "warning: resistor {} has zero resistance; it is stamped as {:e} ohm",
                r.name,
                rustspice_core::soa::ZERO_OHM_SUBSTITUTE
            );
        }
    }
    if strict && n_warnings > 0 {
        return Err(format!(
            "{n_warnings} reader warning(s) treated as errors (--strict): the deck uses constructs RustSpice ignores or approximates"
        )
        .into());
    }
    let t1 = Instant::now();
    let mut sim = Simulator::from_netlist(parsed, backend_arc)?;
    sim.set_limits(*limits);
    let compile_us = t1.elapsed().as_secs_f64() * 1e6;
    stats.compile_us = Some(compile_us);
    stats.solver_kind = Some(format!("{:?}", sim.linear_solver.kind));

    println!(
        "=== RustSpice Simulation: {} (Backend: {}, Solver: {:?}, Compile: {:.1} us) ===",
        sim.netlist.title,
        sim.backend.backend_name(),
        sim.linear_solver.kind,
        compile_us
    );

    let labels = mna_labels(&sim);
    let mut plots: Vec<RawPlotOut> = Vec::new();
    let analyses = sim.netlist.analyses.clone();
    let mut total_us = 0.0;
    let mut total_pts = 0u64;
    let mut kinds: Vec<&'static str> = Vec::new();
    for cmd in analyses {
        match cmd {
            AnalysisCmd::Op => {
                let t_run = Instant::now();
                let res = sim.run_dc_op()?;
                let run_us = t_run.elapsed().as_secs_f64() * 1e6;
                println!(
                    "[DC OP] Converged in {} iterations ({:.1} us, max residual = {:.3e} A)",
                    res.iterations, run_us, res.max_residual
                );
                for node_id in 1..sim.netlist.node_names.len().min(16) {
                    let name = &sim.netlist.node_names[node_id];
                    println!("  V({name}) = {:.9} V", res.solution[node_id - 1]);
                }
                stats.analyses.push(AnalysisStat {
                    analysis: "op",
                    analysis_us: Some(run_us),
                    points: Some(1),
                });
                total_us += run_us;
                total_pts += 1;
                kinds.push("op");
                if raw.is_some() {
                    plots.push(RawPlotOut {
                        plotname: "Operating Point",
                        vars: raw_vars(&labels),
                        axis: None,
                        rows: vec![res.solution],
                    });
                }
            }
            AnalysisCmd::Dc {
                source_name,
                start,
                stop,
                step,
            } => {
                let t_run = Instant::now();
                let res = sim.run_dc_sweep(&source_name, start, stop, step)?;
                let run_us = t_run.elapsed().as_secs_f64() * 1e6;
                println!(
                    "[DC Sweep] {} points on `{}` ({} total NR iterations, {:.1} us)",
                    res.sweep_values.len(),
                    source_name,
                    res.total_iterations,
                    run_us
                );
                let n = res.sweep_values.len() as u64;
                stats.analyses.push(AnalysisStat {
                    analysis: "dc",
                    analysis_us: Some(run_us),
                    points: Some(n),
                });
                total_us += run_us;
                total_pts += n;
                kinds.push("dc");
                if raw.is_some() {
                    // ngspice names the sweep vector `v(v-sweep)` / `i(i-sweep)` after the source type
                    let k = if source_name.to_ascii_lowercase().starts_with('i') {
                        "i"
                    } else {
                        "v"
                    };
                    let mut vars = vec![bench::rawfile::Var {
                        name: format!("{k}({k}-sweep)"),
                        kind: if k == "i" { "current" } else { "voltage" }.to_string(),
                    }];
                    vars.extend(raw_vars(&labels));
                    plots.push(RawPlotOut {
                        plotname: "DC transfer characteristic",
                        vars,
                        axis: Some(res.sweep_values),
                        rows: res.solutions,
                    });
                }
            }
            AnalysisCmd::Tran { tstep, tstop, .. } => {
                let t_run = Instant::now();
                let res = sim.run_transient(tstep, tstop)?;
                let run_us = t_run.elapsed().as_secs_f64() * 1e6;
                println!(
                    "[Transient] {} time points up to {:.3e}s ({} NR iters, {} LU factorizations, {:.1} us)",
                    res.time_points.len(),
                    tstop,
                    res.total_nr_iterations,
                    res.lu_factorizations,
                    run_us
                );
                let n = res.time_points.len() as u64;
                stats.analyses.push(AnalysisStat {
                    analysis: "tran",
                    analysis_us: Some(run_us),
                    points: Some(n),
                });
                total_us += run_us;
                total_pts += n;
                kinds.push("tran");
                if raw.is_some() {
                    let mut vars = vec![bench::rawfile::Var {
                        name: "time".to_string(),
                        kind: "time".to_string(),
                    }];
                    vars.extend(raw_vars(&labels));
                    plots.push(RawPlotOut {
                        plotname: "Transient Analysis",
                        vars,
                        axis: Some(res.time_points),
                        rows: res.waveforms,
                    });
                }
            }
        }
    }
    if let Some(from) = sim.linear_solver.demoted_from {
        // The unrolled LU's frozen pivot order became unstable and the pivoting tier took over.
        stats.solver_kind = Some(format!("{:?}>{:?}", from, sim.linear_solver.kind));
        eprintln!(
            "note: {:?} pivot order became numerically unstable; continued with {:?}",
            from, sim.linear_solver.kind
        );
    }
    if !kinds.is_empty() {
        stats.analysis = Some(kinds.join("+"));
        stats.analysis_us = Some(total_us);
        stats.points = Some(total_pts);
    }
    if let Some(raw_path) = raw {
        let t_w = Instant::now();
        let tmp = raw_path.with_extension("raw.partial");
        let written = (|| -> std::io::Result<()> {
            {
                let f = fs::File::create(&tmp)?;
                let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
                for p in &plots {
                    bench::rawfile::write_rows(
                        &mut w,
                        &sim.netlist.title,
                        p.plotname,
                        &p.vars,
                        p.axis.as_deref(),
                        &p.rows,
                    )?;
                }
                std::io::Write::flush(&mut w)?;
            }
            fs::rename(&tmp, raw_path)
        })();
        if let Err(source) = written {
            let _ = fs::remove_file(&tmp);
            return Err(Box::new(RawWriteError {
                path: raw_path.to_path_buf(),
                source,
            }));
        }
        stats.write_us = Some(t_w.elapsed().as_secs_f64() * 1e6);
    }
    Ok(())
}

/// The `--raw` file could not be written (disk full, read-only or missing directory). Typed so that `main` can exit
/// with [`bench::rustsim::HARNESS_IO_EXIT`] instead of the generic error code of a simulation failure.
#[derive(Debug)]
struct RawWriteError {
    path: PathBuf,
    source: std::io::Error,
}

impl std::fmt::Display for RawWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot write rawfile {}: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for RawWriteError {}
