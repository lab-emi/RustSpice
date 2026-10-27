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

## Head-to-head vs. `ngspice-47` (KLU)

| Benchmark suite | Circuit scale | `ngspice` wall, ms ↓ | `RustSpice` total, ms ↓ | Max \|ΔV\|, V ↓ | End-to-end speedup ↑ | Solver speedup ↑ |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: |
| **5,000-Node Power Grid (DC OP)** | 10,000 Resistors, 5,002 MNA vars | 58.47 | **11.053** | `2.16e-45` | **5.3×** | **243.2×** |
| **2,000-Node RC Line (500-Step Tran)** | 4,000 R/C elements, 500 steps | 244.81 | **32.218** | `0.00e0` | **7.6×** | **8.6×** |
| **64-Diode Nonlinear Array (DC Sweep)** | 64 Diodes + 128 R, 201 points | 8.71 | **2.898** | `2.15e-14` | **3.0×** | **1.9×** |
| **100-Stage CMOS Inverter Chain (Tran)** | 200 MOSFETs + 100 C, 400 steps | 42.14 | **16.265** | `3.31e-12` | **2.6×** | **2.5×** |
| **200-Param Sensitivity (Adjoint vs. FD)** | 200 Resistor Gradients | 2,167.17 | **0.345** | `1.50e-11` | **6,284.9×** | **9,867.0×** |
| **128-Corner PVT Batch (Rayon Parallel)** | 128 Nonlinear 32-Diode Corners | 686.08 | **2.136** | `0.00e0` | **321.1×** | **131.9×** |

## Guides and reference

- [Architecture and crate organization](architecture.md)
- [Benchmark report and methodology](benchmark_report.md)
