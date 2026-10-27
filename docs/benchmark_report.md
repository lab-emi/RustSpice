# Benchmarks: RustSpice vs. ngspice-47 (KLU)

> **Hardware environment**: Linux x86_64 (32 hardware threads, AVX-512/AVX2 + NVIDIA GeForce RTX 4090 Laptop GPU)  
> **Reference baseline**: `ngspice-47` compiled with the KLU direct sparse solver (`ngspice -b`)

## Head-to-head benchmark results

| Benchmark suite | Circuit scale | `ngspice` wall, ms ↓ | `ngspice` solver, ms ↓ | `RustSpice` total, ms ↓ | `RustSpice` solver, ms ↓ | Max \|ΔV\|, V ↓ | End-to-end speedup ↑ | Solver kernel speedup ↑ |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **1. 5,000-Node Power Grid (DC OP)** | 10,000 Resistors, 5,002 MNA variables | 58.47 | 50.00 | **11.053** | **0.206** | `2.16e-45` | **5.3×** | **243.2×** |
| **2. 2,000-Node RC Line (500-Step Tran)** | 4,000 R/C elements, 500 time steps | 244.81 | 238.00 | **32.218** | **27.836** | `0.00e0` | **7.6×** | **8.6×** |
| **3. 64-Diode Nonlinear Array (201-Pt DC Sweep)** | 64 Diodes + 128 Resistors, 201 sweep points | 8.71 | 5.00 | **2.898** | **2.664** | `2.15e-14` | **3.0×** | **1.9×** |
| **4. 100-Stage CMOS Inverter Chain (Tran)** | 200 NMOS/PMOS + 100 Caps, 400 time steps | 42.14 | 39.00 | **16.265** | **15.857** | `3.31e-12` | **2.6×** | **2.5×** |
| **5. 200-Param Sensitivity (Adjoint vs. FD)** | 200 Resistor Gradients (1 Adjoint Solve vs. 400 FD) | 2,167.17 | 800.00 | **0.345** | **0.081** | `1.50e-11` | **6,284.9×** | **9,867.0×** |
| **6. 128-Corner PVT Batch (Rayon Parallel)** | 128 Nonlinear 32-Diode Corners in Parallel | 686.08 | 256.00 | **2.136** | **1.940** | `0.00e0` | **321.1×** | **131.9×** |

## Measurement scope and reproduction

- **End-to-end wall time (`RustSpice` total vs. `ngspice` wall)**: includes netlist parsing, topology compilation, Compressed Sparse Column (CSC) construction, symbolic analysis, device stamping, and numerical linear/nonlinear solve.
- **Solver kernel time (`RustSpice` solver vs. `ngspice` solver)**: compares the core simulation loop against `ngspice` internal `rusage` analysis time.
- **Numerical verification**: every benchmark compares node voltages or parameter sensitivities directly against `ngspice-47` (`set numdgt=12`, `reltol=1e-9`, `vntol=1e-12`, `abstol=1e-14`) or central finite differences.

Reproduce the benchmark suite locally with an installed `ngspice` binary:

```sh
cargo run --release -p rustspice-cli -- bench-ngspice --output docs/benchmark_report.md
```

## Key architectural speedup drivers

1. **Zero-Indirection Symbolic LU & Cranelift Native Machine Code JIT (`rustspice-backend-cpu::jit`)**: eliminates sparse row/column index lookups and dynamic pivoting loops after initial topology compilation.
2. **Static-Base Conflict-Free Gather Table (`CscGatherTable`)**: pre-stamps all linear conductances once during compilation and gathers only active dynamic/nonlinear terms via contiguous indices with zero atomic contention.
3. **Constant-step LU Factorization Caching (`TransientSolver`)**: reuses factored `L` and `U` matrices across fixed-timestep linear transient steps, reducing each time step to a single `O(NNZ)` forward/backward substitution.
4. **Secant Trajectory Predictor & Shamanskii NR Reuse (`NonlinearSolver`)**: extrapolates initial guesses across `.dc` sweeps and `.tran` steps, cutting Newton-Raphson iterations per step by more than 50%.
5. **Discrete Adjoint Sensitivity (`AdjointSensitivityEngine`)**: computes exact gradients with respect to `P` parameters in one transposed triangular solve (`J^T λ = ∂L/∂V`) instead of `2P` perturbed finite-difference simulations.
