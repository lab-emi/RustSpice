# RustSpice

RustSpice simulates transistor-level SPICE netlists (`.op`, `.dc`, `.tran`) and computes discrete adjoint parameter sensitivities with Structure-of-Arrays (SoA) device evaluation, auto-differentiated model kernels and topology-specialized sparse linear solvers.

[All RustEDA modules](https://lab-emi.github.io/RustEDA/)

<picture>
  <source media="(prefers-reduced-motion: reduce)" srcset="assets/rustspice-banner.svg">
  <img src="assets/rustspice-banner.gif" alt="Circuit simulation for the next design iteration." width="1200">
</picture>

## Simulate. Differentiate. Size.

Run DC operating point, DC sweep, transient and discrete adjoint sensitivity analysis from standard SPICE decks. All working vectors and sparse matrix structures are pre-compiled once per topology for zero-allocation hot loops.

```sh
rustspice run examples/cmos_ring_oscillator.sp --backend cpu
rustspice adjoint examples/cmos_ring_oscillator.sp --target-node out
rustspice codegen --model mosfet-ekv --target msl
rustspice bench-ngspice --output target/bench-ngspice.md
```

## Benchmarks

RustEDA publishes one benchmark: a matched Ibex RTL-to-GDS comparison with LibreLane on IHP SG13G2 / EZ130. See the [RustEDA benchmarks](https://lab-emi.github.io/RustEDA/benchmarks/). The [benchmark method](benchmark_report.md) describes how to measure RustSpice against `ngspice` on your own host.

## Batched DC operating points on NVIDIA GPUs

`Simulator::run_batch_dc_op_with` solves one circuit at many parameter corners (supply, PVT or Monte Carlo) on a CUDA GPU, one corner per thread in FP64. The kernel is the CPU solver's own Newton loop, Jacobian gather tables and static-pivot LU, written once in Rust: the normal build runs it with rayon (`cpu_ref`) and [cuda-oxide](https://github.com/NVIDIA/cuda-rust) compiles the same source to PTX. A corner the kernel does not converge is re-solved by the CPU solver, so every corner meets the CPU's convergence criteria. The kernel covers plain diodes and level-1 MOSFETs with their bulk junctions; circuits outside that scope (MOS levels above 1, diodes with breakdown or series resistance, polynomial sources, `.nodeset`) run on the CPU. In the last recorded run, the GPU agreed with the CPU build to `5.6e-16` and with the CPU solver to `1.8e-9` (relative). Creating the CUDA context and JIT-compiling the PTX is a one-time cost per process, so small batches can run faster on the CPU.

## Guides and reference

- [Architecture and crate organization](architecture.md)
- [Benchmark method vs. ngspice-47](benchmark_report.md)
- [Batched DC operating points: CPU vs CUDA method](gpu_batch_report.md)
