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
rustspice bench-ngspice --output docs/benchmark_report.md
```

## Head-to-head vs. `ngspice-47`

| Benchmark | Circuit scale | `ngspice-47`<br>end to end ↓ | `RustSpice`<br>end to end ↓ | Speedup ↑ | Max error: `RustSpice` / `ngspice` default ↓ |
| :--- | :--- | ---: | ---: | ---: | ---: |
| **1. Resistor ladder (DC OP)** | 10,000 resistors, 5,002 MNA unknowns (DC operating point) | 25.98 ms | 11.13 ms | **2.33×** | `1.91e-13` / `1.93e-13` V |
| **2. RC line (transient)** | 4,000 R/C elements, `.tran 10p 5n` (500 steps) | 114 ms | 31.45 ms | **3.62×** (at **8.2× lower error** than ngspice default) | `2.01e-4` / `1.65e-3` of swing (vs tight ngspice) |
| **3. Diode array (DC sweep)** | 64 diodes + 128 resistors, 201-point DC sweep | 6.54 ms | 3.35 ms | **1.95×** | `5.89e-11` / `5.44e-5` V |
| **4. CMOS inverter chain (transient)** | 200 MOSFETs + 100 capacitors, `.tran 5p 2n` (400 steps) | 35.60 ms | 32.12 ms | **1.11×** (at **5.2× lower error** than ngspice default) | `7.40e-2` / `3.86e-1` of swing (vs tight ngspice) |
| **5. Sensitivity (adjoint vs. ngspice `.sens`)** | 200 resistor gradients (one adjoint solve) | 12.55 ms | 1.30 ms | **9.66×** vs `.sens` (**306×** vs FD loop) | `9.33e-15` / `1.00e-6` (normalised) |
| **6. PVT batch (128 corners)** | 128 PVT corners of a 32-diode nonlinear net (Rayon) | 4.30 ms | 3.46 ms | **1.24×** (parallel workload; **2.57×** via `.dc` sweep) | `2.47e-11` / `1.63e-5` V |

Speedup = `ngspice-47` / `RustSpice`, median of 9 interleaved repetitions against the faster of ngspice's SPARSE 1.3 and KLU solvers; both tools run as subprocesses with rawfile output on tmpfs, the single-core benchmarks pinned to one idle core (the parallel batch, benchmark 6, is not pinned). The previously published 5.3×, 7.6×, 3.0×, 2.6×, 6,285× and 321× came from a harness with known defects (1 ms-quantised solver times, a cold process compared with a warm in-process call, hard-coded zero errors, single runs multiplied by 128 or 400, a transient compared at t = 0 only) and are withdrawn; the report lists each one with its replacement.

## Batched DC operating points on NVIDIA GPUs

`Simulator::run_batch_dc_op_with` solves one circuit at many parameter corners (supply, PVT or Monte Carlo) on a CUDA GPU, one corner per thread in FP64. The kernel is the CPU solver's own Newton loop, Jacobian gather tables and static-pivot LU, written once in Rust: the normal build runs it with rayon (`cpu_ref`) and [cuda-oxide](https://github.com/NVIDIA/cuda-rust) compiles the same source to PTX. A corner the kernel does not converge is re-solved by the CPU solver, so every corner meets the CPU's convergence criteria. The kernel covers plain diodes and level-1 MOSFETs with their bulk junctions; circuits outside that scope (MOS levels above 1, diodes with breakdown or series resistance, polynomial sources, `.nodeset`) run on the CPU. On an RTX 4090 Laptop GPU, 1,048,576 supply corners of the 32-diode cascade of benchmark 6 take 535 ms against 1,575 ms for the same kernel on 32 CPU threads (2.95×), and 131,072 corners of the 100-stage CMOS inverter chain of benchmark 4 take 300 ms against 617 ms (2.05×), agreeing with the CPU build to `5.6e-16` and with the CPU solver to `1.8e-9` (relative). Below about 10,000 corners the CPU is faster, and creating the CUDA context and JIT-compiling the PTX costs about 0.3 s per process.

## Guides and reference

- [Architecture and crate organization](architecture.md)
- [Benchmark report and methodology](benchmark_report.md)
- [Batched DC operating points: CPU vs CUDA](gpu_batch_report.md)
