//! Multi-threaded `rayon` Batch Execution Engine for parameter sweeps, PVT corners,
//! Monte Carlo statistical verification, and AI4EDA reinforcement learning rollouts.

use rayon::prelude::*;
use rustspice_core::{CompiledTopology, SimResult};

/// Parameter override `(device_name, param_name, value)` for a single batch instance.
#[derive(Clone, Debug)]
pub struct BatchCircuitInput {
    pub overrides: Vec<(String, String, f64)>,
}

/// Output from a single circuit instance in a parallel batch simulation.
#[derive(Clone, Debug)]
pub struct BatchCircuitOutput {
    pub solution: Vec<f64>,
    pub iterations: usize,
}

/// Parallel batch executor utilizing `rayon` work-stealing thread pools.
#[derive(Clone, Debug, Default)]
pub struct BatchCpuExecutor;

impl BatchCpuExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates a closure across `inputs` in parallel using thread-local cloned [`CompiledTopology`]
    /// instances with zero shared mutable state or lock contention.
    pub fn execute_batch<F>(
        &self,
        base_topology: &CompiledTopology,
        inputs: &[BatchCircuitInput],
        solve_fn: F,
    ) -> SimResult<Vec<BatchCircuitOutput>>
    where
        F: Fn(&mut CompiledTopology) -> SimResult<BatchCircuitOutput> + Sync,
    {
        inputs
            .par_iter()
            .map(|inp| {
                let mut local_topo = base_topology.clone();
                for (dev, param, val) in &inp.overrides {
                    local_topo.soa.set_device_parameter(dev, param, *val)?;
                }
                solve_fn(&mut local_topo)
            })
            .collect()
    }
}
