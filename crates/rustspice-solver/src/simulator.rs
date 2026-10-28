//! High-level [`Simulator`] facade orchestrating netlist parsing, topology compilation,
//! hardware backend dispatch, and DC / Transient / Adjoint / Batch execution.

use std::sync::Arc;

use rustspice_backend_cpu::{
    BatchCircuitInput, BatchCircuitOutput, BatchCpuExecutor, CpuSimdBackend,
};
use rustspice_core::{
    compile_netlist, parse_netlist, parse_netlist_with_limits, CompiledTopology, ComputeBackend,
    Netlist, ResourceLimits, SimError, SimResult,
};

use crate::adjoint::{AdjointGradientResult, AdjointSensitivityEngine, ParameterGradSpec};
use crate::linear::AdaptiveSparseSolver;
use crate::nonlinear::{DcOpResult, DcSweepResult, NonlinearSolver, NrOptions};
use crate::transient::{TranOptions, TranResult, TransientSolver};

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
        let mut topology = compile_netlist(&netlist)?;
        let dim = topology.soa.mna_dim;
        let mut init_rhs = vec![0.0_f64; dim];
        let init_x = vec![0.0_f64; dim];
        backend.evaluate_devices(&mut topology.soa, &init_x)?;
        backend.stamp_jacobian(&topology.soa, &mut topology.jacobian, &mut init_rhs)?;

        let linear_solver = AdaptiveSparseSolver::compile(&topology.jacobian)?;
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

    /// Solves a Transient simulation (`.tran <tstep> <tstop>`).
    pub fn run_transient(&mut self, tstep: f64, tstop: f64) -> SimResult<TranResult> {
        self.transient_solver.solve_transient(
            &mut self.topology,
            self.backend.as_ref(),
            &mut self.linear_solver,
            &mut self.nonlinear_solver,
            tstep,
            tstop,
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
