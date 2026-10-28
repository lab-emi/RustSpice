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
| **1. Resistor ladder (DC OP)** | 10,000 resistors, 5,002 MNA unknowns (DC operating point) | 23.66 ms | 10.17 ms | **2.33×** | `1.91e-13` / `1.93e-13` V |
| **2. RC line (transient)** | 4,000 R/C elements, `.tran 10p 5n` (500 steps) | 104 ms | 29.29 ms | **3.56×** (at **8.2× lower error** than ngspice default) | `2.01e-4` / `1.65e-3` of swing (vs tight ngspice) |
| **3. Diode array (DC sweep)** | 64 diodes + 128 resistors, 201-point DC sweep | 4.82 ms | 2.55 ms | **1.89×** | `5.89e-11` / `5.44e-5` V |
| **4. CMOS inverter chain (transient)** | 200 MOSFETs + 100 capacitors, `.tran 5p 2n` (400 steps) | 31.65 ms | 28.34 ms | **1.12×** (at **5.2× lower error** than ngspice default) | `7.40e-2` / `3.86e-1` of swing (vs tight ngspice) |
| **5. Sensitivity (adjoint vs. ngspice `.sens`)** | 200 resistor gradients (one adjoint solve) | 11.16 ms | 1.08 ms | **10.3×** vs `.sens` (**333×** vs FD loop) | `9.33e-15` / `1.00e-6` (normalised) |
| **6. PVT batch (128 corners)** | 128 PVT corners of a 32-diode nonlinear net (Rayon) | 3.69 ms | 3.56 ms | **1.03×** (parallel workload; **2.71×** via `.dc` sweep) | `2.47e-11` / `1.63e-5` V |

Speedup = `ngspice-47` / `RustSpice`, median of 9 interleaved repetitions against the faster of ngspice's SPARSE 1.3 and KLU solvers; both tools run as subprocesses with rawfile output on tmpfs, the single-core benchmarks pinned to one idle core (the parallel batch, benchmark 6, is not pinned). The previously published 5.3×, 7.6×, 3.0×, 2.6×, 6,285× and 321× came from a harness with known defects (1 ms-quantised solver times, a cold process compared with a warm in-process call, hard-coded zero errors, single runs multiplied by 128 or 400, a transient compared at t = 0 only) and are withdrawn; the report lists each one with its replacement.

## Guides and reference

- [Architecture and crate organization](architecture.md)
- [Benchmark report and methodology](benchmark_report.md)
