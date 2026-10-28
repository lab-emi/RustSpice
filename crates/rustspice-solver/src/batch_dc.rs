//! Batched DC operating points on the batch kernel of `rustspice-backend-cuda-simt` (its rayon `cpu_ref` or a
//! CUDA GPU), with the CPU solver for what the kernel leaves.
//!
//! The kernel replays the CPU solver's own data (gather tables, static-pivot LU compiler, Newton tolerances) without
//! the CPU's fallbacks: a corner that does not converge, needs a new pivot order or puts a junction beyond the
//! exponential's range is re-solved by [`Simulator::run_batch_dc_op`], which has dynamic re-pivoting, gmin stepping
//! and source stepping. So every corner meets the same criteria as on the CPU. Its column order is the CPU's with
//! the nodes of grounded voltage sources first ([`AdaptiveSparseSolver::static_pivot_plan`]), which keeps one
//! static pivot order stable through the regimes of a MOSFET circuit.

use rustc_hash::FxHashMap;
use rustspice_backend_cpu::{BatchCircuitInput, BatchCircuitOutput};
use rustspice_backend_cuda_simt::batch::{
    solve_cpu_ref, BatchPlan, BatchSolution, CudaBatchSolver, NewtonSettings, ParamSlot,
};
use rustspice_core::{DeviceSoaBuffer, SimError, SimResult};

use crate::linear::AdaptiveSparseSolver;
use crate::nonlinear::NonlinearSolver;
use crate::simulator::Simulator;

/// Where the batch kernel runs.
#[derive(Clone, Copy)]
pub enum BatchDcEngine<'a> {
    /// The rayon `cpu_ref` of the kernel.
    CpuRef,
    /// A CUDA GPU.
    Cuda(&'a CudaBatchSolver),
}

/// Result of [`Simulator::run_batch_dc_op_with`].
#[derive(Clone, Debug)]
pub struct BatchDcRun {
    pub outputs: Vec<BatchCircuitOutput>,
    /// Corners the kernel did not converge and the CPU solver re-solved.
    pub cpu_fallback: usize,
    /// Why the whole batch ran on the CPU solver instead of the kernel (`None` when the kernel ran).
    pub unsupported: Option<String>,
}

/// What a `(device, parameter)` override changes (the dispatch of `DeviceSoaBuffer::set_device_parameter`).
#[derive(Clone, Copy)]
enum Target {
    Resistor(u32),
    Source(u32),
    DiodeIs(u32),
    DiodeN(u32),
    DiodeArea(u32),
    /// A MOSFET value set as given (`W`, `L`, `VTO`, `KP`, `LAMBDA`).
    Mos(ParamSlot),
}

fn resolve(soa: &DeviceSoaBuffer, dev: &str, param: &str) -> Result<Target, String> {
    let p = param.to_ascii_lowercase();
    let find = |names: &[String]| names.iter().position(|n| n.eq_ignore_ascii_case(dev));
    if let Some(i) = find(&soa.res_names) {
        if p == "r" || p == "resistance" {
            return Ok(Target::Resistor(soa.res_cond_buf_idx[i]));
        }
    }
    if let Some(m) = find(&soa.mos_names) {
        let m = m as u32;
        return match p.as_str() {
            "w" => Ok(Target::Mos(ParamSlot::MosW(m))),
            "l" => Ok(Target::Mos(ParamSlot::MosL(m))),
            "vto" | "vt0" => Ok(Target::Mos(ParamSlot::MosVto(m))),
            "kp" => Ok(Target::Mos(ParamSlot::MosKp(m))),
            "lambda" => Ok(Target::Mos(ParamSlot::MosLambda(m))),
            _ => Err(format!("unsupported MOSFET parameter `{param}`")),
        };
    }
    if let Some(d) = find(&soa.diode_names) {
        return match p.as_str() {
            "is" => Ok(Target::DiodeIs(d as u32)),
            "n" => Ok(Target::DiodeN(d as u32)),
            "area" => Ok(Target::DiodeArea(d as u32)),
            _ => Err(format!("unsupported diode parameter `{param}`")),
        };
    }
    if p == "dc" || p == "v" || p == "i" {
        if let Some(i) = find(&soa.vsrc_names) {
            return Ok(Target::Source(soa.vsrc_rhs_buf_idx[i]));
        }
        if let Some(i) = find(&soa.isrc_names) {
            return Ok(Target::Source(soa.isrc_rhs_buf_idx[i]));
        }
    }
    Err(format!("`{dev}` / `{param}` is not a batch parameter"))
}

impl Simulator {
    /// The batch kernel's plan of this circuit and the slot-major parameter values of `inputs`
    /// (`params[j * inputs.len() + c]`), or why the kernel cannot solve it.
    pub fn batch_dc_plan(
        &self,
        inputs: &[BatchCircuitInput],
    ) -> Result<(BatchPlan, Vec<f64>), String> {
        // the DC state at x = 0, as `solve_dc_op` starts from it
        let mut topo = self.topology.clone();
        topo.soa.is_transient = false;
        topo.soa.update_sources_at_time(None);
        NonlinearSolver::check_dc_solvable(&topo.soa).map_err(|e| e.to_string())?;
        if !topo.soa.isrc_names.is_empty() && !topo.soa.dc_floating_diag_idx.is_empty() {
            return Err(
                "current sources and DC-floating nodes: solvability depends on the corner".into(),
            );
        }
        let dim = topo.soa.mna_dim;
        let x0 = vec![0.0; dim];
        let mut rhs = vec![0.0; dim];
        NonlinearSolver::sync_device_voltages(&mut topo, &x0);
        self.backend
            .evaluate_devices(&mut topo.soa, &x0)
            .map_err(|e| e.to_string())?;
        self.backend
            .stamp_jacobian(&topo.soa, &mut topo.jacobian, &mut rhs)
            .map_err(|e| e.to_string())?;

        let soa = &topo.soa;
        let mut slots: Vec<ParamSlot> = Vec::new();
        let mut slot_of: FxHashMap<ParamSlot, usize> = FxHashMap::default();
        let mut slot = |s: ParamSlot, slots: &mut Vec<ParamSlot>| {
            *slot_of.entry(s).or_insert_with(|| {
                slots.push(s);
                slots.len() - 1
            })
        };
        // every override resolved once: corners usually list the same overrides in the same order, so the one at
        // the same position of the previous corner is tried first (no allocation per override)
        let mut flat: Vec<(Target, usize, f64)> = Vec::new();
        let mut ends: Vec<usize> = Vec::with_capacity(inputs.len());
        let mut resolved: FxHashMap<(String, String), Target> = FxHashMap::default();
        let mut at_pos: Vec<Option<(&str, &str, Target, usize)>> = Vec::new();
        for inp in inputs {
            for (j, (dev, param, v)) in inp.overrides.iter().enumerate() {
                if at_pos.len() <= j {
                    at_pos.push(None);
                }
                let (target, s) = match at_pos[j] {
                    Some((d, p, t, s))
                        if d.eq_ignore_ascii_case(dev) && p.eq_ignore_ascii_case(param) =>
                    {
                        (t, s)
                    }
                    _ => {
                        let key = (dev.to_ascii_lowercase(), param.to_ascii_lowercase());
                        let t = match resolved.get(&key) {
                            Some(&t) => t,
                            None => {
                                let t = resolve(soa, dev, param)?;
                                resolved.insert(key, t);
                                t
                            }
                        };
                        let s = match t {
                            Target::Resistor(i) => slot(ParamSlot::Conductance(i), &mut slots),
                            Target::Source(i) => slot(ParamSlot::SourceValue(i), &mut slots),
                            Target::DiodeIs(d) | Target::DiodeArea(d) => {
                                slot(ParamSlot::DiodeIs(d), &mut slots)
                            }
                            Target::DiodeN(d) => slot(ParamSlot::DiodeN(d), &mut slots),
                            Target::Mos(p) => slot(p, &mut slots),
                        };
                        at_pos[j] = Some((dev.as_str(), param.as_str(), t, s));
                        (t, s)
                    }
                };
                flat.push((target, s, *v));
            }
            ends.push(flat.len());
        }
        let base: Vec<f64> = slots
            .iter()
            .map(|s| match *s {
                ParamSlot::Conductance(i) => soa.cond_buf[i as usize],
                ParamSlot::SourceValue(i) => soa.rhs_buf[i as usize],
                ParamSlot::DiodeIs(d) => soa.diode_is[d as usize],
                ParamSlot::DiodeN(d) => soa.diode_n[d as usize],
                ParamSlot::MosW(m) => soa.mos_w[m as usize],
                ParamSlot::MosL(m) => soa.mos_l[m as usize],
                ParamSlot::MosVto(m) => soa.mos_vto[m as usize],
                ParamSlot::MosKp(m) => soa.mos_kp[m as usize],
                ParamSlot::MosLambda(m) => soa.mos_lambda[m as usize],
            })
            .collect();
        let corners = inputs.len();
        let mut params = vec![0.0; slots.len() * corners];
        let mut vals = base.clone();
        let mut area: FxHashMap<u32, f64> = FxHashMap::default();
        let mut start = 0;
        for (c, &end) in ends.iter().enumerate() {
            vals.copy_from_slice(&base);
            area.clear();
            // applied in order, with the arithmetic of `set_device_parameter`
            for &(target, s, v) in &flat[start..end] {
                match target {
                    Target::Resistor(_) => vals[s] = DeviceSoaBuffer::resistor_conductance("", v),
                    Target::Source(_) | Target::DiodeIs(_) | Target::DiodeN(_) | Target::Mos(_) => {
                        vals[s] = v
                    }
                    Target::DiodeArea(d) => {
                        let old = *area.get(&d).unwrap_or(&soa.diode_area[d as usize]);
                        if old > 0.0 {
                            vals[s] *= v / old;
                        }
                        area.insert(d, v);
                    }
                }
            }
            for (j, &v) in vals.iter().enumerate() {
                params[j * corners + c] = v;
            }
            start = end;
        }

        let o = &self.nonlinear_solver.options;
        let newton = NewtonSettings {
            max_iters: o.max_iters,
            reltol: o.reltol,
            vntol: o.vntol,
            abstol: o.abstol,
            res_reltol: o.res_reltol,
            box_release_iters: o.box_release_iters,
        };
        // the static pivot order of these values, without the entries that stay zero
        let constant_zero = BatchPlan::constant_zero_entries(&topo, &slots);
        let (perm, value_map, lu) =
            AdaptiveSparseSolver::static_pivot_plan(&topo.jacobian, &constant_zero).ok_or(
                "the circuit is beyond the unrolled LU tier (dimension or operation budget)",
            )?;
        let plan = BatchPlan::build(&topo, &perm, &value_map, &lu, &newton, &slots)?;
        Ok((plan, params))
    }

    /// [`Self::run_batch_dc_op`] on the batch kernel: the same operating points, the kernel's unconverged corners
    /// re-solved by the CPU solver. A circuit the kernel does not support runs entirely on the CPU solver
    /// ([`BatchDcRun::unsupported`] says why).
    pub fn run_batch_dc_op_with(
        &self,
        inputs: &[BatchCircuitInput],
        engine: BatchDcEngine<'_>,
    ) -> SimResult<BatchDcRun> {
        let (plan, params) = match self.batch_dc_plan(inputs) {
            Ok(p) => p,
            Err(reason) => {
                return Ok(BatchDcRun {
                    outputs: self.run_batch_dc_op(inputs)?,
                    cpu_fallback: inputs.len(),
                    unsupported: Some(reason),
                })
            }
        };
        let corners = inputs.len();
        let sol: BatchSolution = match engine {
            BatchDcEngine::CpuRef => solve_cpu_ref(&plan, &params, corners),
            BatchDcEngine::Cuda(gpu) => {
                gpu.solve(&plan, &params, corners, None)
                    .map_err(|e| SimError::BackendError {
                        backend: "cuda-batch".to_string(),
                        message: e.to_string(),
                    })?
            }
        };
        let mut outputs: Vec<BatchCircuitOutput> = (0..corners)
            .map(|c| BatchCircuitOutput {
                solution: sol.corner(c).to_vec(),
                iterations: sol.iterations[c] as usize,
            })
            .collect();
        let redo: Vec<usize> = sol.unconverged().collect();
        if !redo.is_empty() {
            let sub: Vec<BatchCircuitInput> = redo.iter().map(|&c| inputs[c].clone()).collect();
            for (c, out) in redo.iter().zip(self.run_batch_dc_op(&sub)?) {
                outputs[*c] = out;
            }
        }
        Ok(BatchDcRun {
            outputs,
            cpu_fallback: redo.len(),
            unsupported: None,
        })
    }
}
