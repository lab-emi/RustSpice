//! High-level [`Simulator`] facade orchestrating netlist parsing, topology compilation,
//! hardware backend dispatch, and DC / Transient / Adjoint / Batch execution.

use std::sync::Arc;

use rustspice_backend_cpu::{
    BatchCircuitInput, BatchCircuitOutput, BatchCpuExecutor, CpuSimdBackend,
};
use rustspice_core::{
    compile_netlist, parse_netlist, parse_netlist_with_limits, CompiledTopology, ComputeBackend,
    DcSweep, Netlist, ResourceLimits, SimError, SimResult,
};

use crate::adjoint::{AdjointGradientResult, AdjointSensitivityEngine, ParameterGradSpec};
use crate::linear::AdaptiveSparseSolver;
use crate::nonlinear::{DcOpResult, DcSweepResult, NonlinearSolver, NrOptions};
use crate::transient::{TranOptions, TranResult, TranSpec, TransientSolver};

#[derive(Clone, Copy, PartialEq)]
enum SweepVar {
    Source,
    Resistor,
    Temp,
}

/// High-level circuit simulator engine.
pub struct Simulator {
    pub netlist: Netlist,
    pub topology: CompiledTopology,
    pub backend: Arc<dyn ComputeBackend>,
    pub linear_solver: AdaptiveSparseSolver,
    pub nonlinear_solver: NonlinearSolver,
    pub transient_solver: TransientSolver,
    pub adjoint_engine: AdjointSensitivityEngine,
}

impl Simulator {
    /// Parses a SPICE netlist string and compiles it with the default [`CpuSimdBackend`].
    pub fn from_spice(spice_src: &str) -> SimResult<Self> {
        let netlist = parse_netlist(spice_src)?;
        Self::from_netlist(netlist, Arc::new(CpuSimdBackend::new()))
    }

    /// Parses a SPICE netlist string and compiles it with a custom [`ComputeBackend`].
    pub fn from_spice_with_backend(
        spice_src: &str,
        backend: Arc<dyn ComputeBackend>,
    ) -> SimResult<Self> {
        let netlist = parse_netlist(spice_src)?;
        Self::from_netlist(netlist, backend)
    }

    /// Compiles a [`Netlist`] with the specified hardware [`ComputeBackend`].
    pub fn from_netlist(netlist: Netlist, backend: Arc<dyn ComputeBackend>) -> SimResult<Self> {
        netlist.validate_finite()?;
        let (topology, linear_solver) = Self::compile_parts(&netlist, backend.as_ref())?;
        let dim = topology.soa.mna_dim;
        let nonlinear_solver = NonlinearSolver::new(dim, NrOptions::default());
        let transient_solver = TransientSolver::new(TranOptions::default());
        let adjoint_engine = AdjointSensitivityEngine::new();

        Ok(Self {
            netlist,
            topology,
            backend,
            linear_solver,
            nonlinear_solver,
            transient_solver,
            adjoint_engine,
        })
    }

    fn compile_parts(
        netlist: &Netlist,
        backend: &dyn ComputeBackend,
    ) -> SimResult<(CompiledTopology, AdaptiveSparseSolver)> {
        let mut topology = compile_netlist(netlist)?;
        let dim = topology.soa.mna_dim;
        let mut init_rhs = vec![0.0_f64; dim];
        let init_x = vec![0.0_f64; dim];
        backend.evaluate_devices(&mut topology.soa, &init_x)?;
        backend.stamp_jacobian(&topology.soa, &mut topology.jacobian, &mut init_rhs)?;
        let linear_solver = AdaptiveSparseSolver::compile(&topology.jacobian)?;
        Ok((topology, linear_solver))
    }

    /// Recompiles the topology from `self.netlist` (after a temperature change).
    fn recompile(&mut self) -> SimResult<()> {
        let (topology, linear_solver) = Self::compile_parts(&self.netlist, self.backend.as_ref())?;
        self.topology = topology;
        self.linear_solver = linear_solver;
        Ok(())
    }

    /// Parses `spice_src` under explicit [`ResourceLimits`] and applies them to the analyses.
    pub fn from_spice_with_limits(
        spice_src: &str,
        backend: Arc<dyn ComputeBackend>,
        limits: ResourceLimits,
    ) -> SimResult<Self> {
        let netlist = parse_netlist_with_limits(spice_src, &limits)?;
        let mut sim = Self::from_netlist(netlist, backend)?;
        sim.set_limits(limits);
        Ok(sim)
    }

    /// Sets the work / memory budgets of the `.dc` and `.tran` analyses.
    pub fn set_limits(&mut self, limits: ResourceLimits) {
        self.nonlinear_solver.limits = limits;
        self.transient_solver.limits = limits;
    }

    /// Solves the DC Operating Point (`.op`).
    pub fn run_dc_op(&mut self) -> SimResult<DcOpResult> {
        self.nonlinear_solver.solve_dc_op(
            &mut self.topology,
            self.backend.as_ref(),
            &mut self.linear_solver,
        )
    }

    /// Solves a DC Sweep (`.dc <source> <start> <stop> <step>`).
    pub fn run_dc_sweep(
        &mut self,
        source_name: &str,
        start: f64,
        stop: f64,
        step: f64,
    ) -> SimResult<DcSweepResult> {
        self.nonlinear_solver.solve_dc_sweep(
            &mut self.topology,
            self.backend.as_ref(),
            &mut self.linear_solver,
            source_name,
            start,
            stop,
            step,
        )
    }

    /// Runs a `.dc` card: `inner` sweeps an independent source, a resistor value or `temp`; an `outer` sweep (nested
    /// `.dc`) repeats it for each of its values, so the rows run over `inner` fastest (the abscissa repeats).
    pub fn run_dc_analysis(
        &mut self,
        inner: &DcSweep,
        outer: Option<&DcSweep>,
    ) -> SimResult<DcSweepResult> {
        let is_source = |n: &str| {
            let soa = &self.topology.soa;
            soa.vsrc_names
                .iter()
                .chain(&soa.isrc_names)
                .any(|s| s.eq_ignore_ascii_case(n))
        };
        if outer.is_none() && is_source(&inner.name) {
            return self.run_dc_sweep(&inner.name, inner.start, inner.stop, inner.step);
        }
        let kind = |n: &str| -> SimResult<SweepVar> {
            if n.eq_ignore_ascii_case("temp") {
                Ok(SweepVar::Temp)
            } else if is_source(n) {
                Ok(SweepVar::Source)
            } else if self
                .netlist
                .resistors
                .iter()
                .any(|r| r.name.eq_ignore_ascii_case(n))
            {
                Ok(SweepVar::Resistor)
            } else {
                Err(SimError::TopologyError(format!(
                    "`.dc {n}`: no independent source or resistor named `{n}` (and it is not `temp`)"
                )))
            }
        };
        let inner_kind = kind(&inner.name)?;
        let outer_kind = outer.map(|o| kind(&o.name)).transpose()?;
        let dim = self.topology.soa.mna_dim;
        let inner_vals =
            self.nonlinear_solver
                .sweep_grid(inner.start, inner.stop, inner.step, dim)?;
        let outer_vals = match outer {
            Some(o) => self
                .nonlinear_solver
                .sweep_grid(o.start, o.stop, o.step, dim)?,
            None => vec![f64::NAN],
        };
        // the whole nested grid is budgeted before any solution is stored
        let total = inner_vals.len() as f64 * outer_vals.len() as f64;
        self.nonlinear_solver
            .limits
            .check_points("dc_sweep_points", total, dim)?;

        let saved_netlist = self.netlist.clone();
        let touches_temp = inner_kind == SweepVar::Temp || outer_kind == Some(SweepVar::Temp);
        let res = (|| -> SimResult<DcSweepResult> {
            let mut out = DcSweepResult {
                sweep_values: Vec::with_capacity(total as usize),
                solutions: Vec::with_capacity(total as usize),
                total_iterations: 0,
            };
            for &ov in &outer_vals {
                if let (Some(o), Some(k)) = (outer, outer_kind) {
                    self.apply_sweep(k, &o.name, ov)?;
                }
                if inner_kind == SweepVar::Source {
                    let r = self.run_dc_sweep(&inner.name, inner.start, inner.stop, inner.step)?;
                    out.sweep_values.extend(r.sweep_values);
                    out.solutions.extend(r.solutions);
                    out.total_iterations += r.total_iterations;
                    continue;
                }
                for &iv in &inner_vals {
                    self.apply_sweep(inner_kind, &inner.name, iv)?;
                    if inner_kind == SweepVar::Temp {
                        // the recompile restored the deck's values: re-apply the outer sweep point
                        if let (Some(o), Some(k)) = (outer, outer_kind) {
                            self.apply_sweep(k, &o.name, ov)?;
                        }
                    }
                    let r = self.run_dc_op()?;
                    out.sweep_values.push(iv);
                    out.solutions.push(r.solution);
                    out.total_iterations += r.iterations;
                }
            }
            Ok(out)
        })();
        // later analyses see the deck as written
        self.netlist = saved_netlist;
        if touches_temp
            || inner_kind == SweepVar::Resistor
            || outer_kind == Some(SweepVar::Resistor)
            || outer_kind == Some(SweepVar::Source)
        {
            self.recompile()?;
        }
        res
    }

    /// Sets one sweep variable to `v`.
    fn apply_sweep(&mut self, kind: SweepVar, name: &str, v: f64) -> SimResult<()> {
        match kind {
            SweepVar::Source => self.topology.soa.set_source_dc_value(name, v),
            SweepVar::Resistor => {
                if !(v.is_finite() && v != 0.0) {
                    return Err(SimError::TopologyError(format!(
                        "`.dc {name}`: resistance {v} is not a usable value"
                    )));
                }
                for r in &mut self.netlist.resistors {
                    if r.name.eq_ignore_ascii_case(name) {
                        r.resistance = v;
                    }
                }
                self.topology.soa.set_device_parameter(name, "r", v)
            }
            SweepVar::Temp => {
                self.netlist.set_temperature(v)?;
                self.recompile()
            }
        }
    }

    /// Solves a Transient simulation (`.tran <tstep> <tstop>`).
    pub fn run_transient(&mut self, tstep: f64, tstop: f64) -> SimResult<TranResult> {
        self.run_transient_spec(&TranSpec::new(tstep, tstop))
    }

    /// Solves a Transient simulation with every field of the `.tran` card (`tstart`, `tmax`, `uic`).
    pub fn run_transient_spec(&mut self, spec: &TranSpec) -> SimResult<TranResult> {
        self.transient_solver.solve_transient_spec(
            &mut self.topology,
            self.backend.as_ref(),
            &mut self.linear_solver,
            &mut self.nonlinear_solver,
            spec,
            &self.netlist.initial_conditions,
        )
    }

    /// Computes Discrete Adjoint gradients $\frac{\mathrm{d}V(\text{node})}{\mathrm{d}\theta}$
    /// at the DC Operating Point.
    pub fn run_dc_adjoint(
        &mut self,
        target_node_name: &str,
        specs: &[ParameterGradSpec],
    ) -> SimResult<(DcOpResult, AdjointGradientResult)> {
        let op = self.run_dc_op()?;
        let node_id = self.netlist.find_node(target_node_name).ok_or_else(|| {
            SimError::TopologyError(format!("Target node `{target_node_name}` not found"))
        })?;
        let dim = self.topology.soa.mna_dim;
        let mut dl_dx = vec![0.0_f64; dim];
        if node_id > 0 {
            dl_dx[node_id - 1] = 1.0;
        }
        let adj = self.adjoint_engine.compute_dc_adjoint_gradients(
            &mut self.topology,
            self.backend.as_ref(),
            &mut self.linear_solver,
            &op.solution,
            &dl_dx,
            specs,
        )?;
        Ok((op, adj))
    }

    /// Executes a parallel batch of DC Operating Point simulations across all CPU cores using `rayon`.
    pub fn run_batch_dc_op(
        &self,
        batch_inputs: &[BatchCircuitInput],
    ) -> SimResult<Vec<BatchCircuitOutput>> {
        let executor = BatchCpuExecutor::new();
        let backend = self.backend.clone();
        let nr_opts = self.nonlinear_solver.options;

        executor.execute_batch(&self.topology, batch_inputs, |local_topo| {
            let mut local_lin = self.linear_solver.clone();
            local_lin.invalidate_factorization();
            let mut local_nl = NonlinearSolver::new(local_topo.soa.mna_dim, nr_opts);
            let res = local_nl.solve_dc_op(local_topo, backend.as_ref(), &mut local_lin)?;
            Ok(BatchCircuitOutput {
                solution: res.solution,
                iterations: res.iterations,
            })
        })
    }

    /// Reads the voltage of a named node from an MNA solution vector `solution`.
    pub fn node_voltage(&self, solution: &[f64], node_name: &str) -> Option<f64> {
        let id = self.netlist.find_node(node_name)?;
        if id == 0 {
            Some(0.0)
        } else {
            solution.get(id - 1).copied()
        }
    }
}
