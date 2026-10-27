<p align="center">
  <img src="docs/assets/rustspice-wordmark.svg" alt="RustSpice" width="360">
</p>

<h1 align="center">Circuit simulation for the next design iteration.</h1>

<p align="center">
  <a href="https://lab-emi.github.io/RustEDA/modules/rustspice/">Documentation</a> ·
  <a href="https://github.com/lab-emi/RustEDA">RustEDA</a> ·
  <a href="#benchmarks">Benchmarks</a> ·
  <a href="LICENSE">Apache-2.0</a>
</p>

<picture>
  <source media="(prefers-reduced-motion: reduce)" srcset="docs/assets/rustspice-banner.svg">
  <img src="docs/assets/rustspice-banner.gif" alt="RustSpice simulates SPICE netlists with Structure-of-Arrays device kernels, topology-specialized sparse solvers and discrete adjoint sensitivities" width="1200">
</picture>

RustSpice simulates transistor-level SPICE netlists (`.op`, `.dc`, `.tran`) and computes discrete adjoint parameter sensitivities with Structure-of-Arrays (SoA) device evaluation, auto-differentiated model kernels and topology-specialized sparse linear solvers.

Part of **[RustEDA](https://github.com/lab-emi/RustEDA)**, infrastructure for chip-design recursive self-improvement. Native tools shorten the work between inference steps; verified results guide the next iteration.

## Benchmarks

**Host: Linux x86_64 (32 hardware threads, AVX-512/AVX2 + NVIDIA GeForce RTX 4090 Laptop GPU)** · Reference baseline: **`ngspice-47` compiled with the KLU direct sparse solver (`ngspice -b`)**.

One warm-up and multiple measured runs; end-to-end wall times include SPICE deck parsing, topology compilation, Compressed Sparse Column (CSC) construction, symbolic factorization, device stamping, and numerical solve. Solver kernel times compare the core simulation loop against `ngspice` internal `rusage` analysis time.

<!-- rustspice-ngspice-benchmark:start -->

| Benchmark suite | Circuit scale | `ngspice-47`<br>Wall, ms ↓ | `ngspice-47`<br>Solver, ms ↓ | `RustSpice`<br>Total, ms ↓ | `RustSpice`<br>Solver, ms ↓ | Max \|ΔV\|,<br>V ↓ | End-to-end<br>Speedup ↑ | Solver kernel<br>Speedup ↑ |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **1. 5,000-Node Power Grid (DC OP)** | 10,000 Resistors, 5,002 MNA variables | 58.47 | 50.00 | **11.053** | **0.206** | `2.16e-45` | **5.3×** | **243.2×** |
| **2. 2,000-Node RC Line (500-Step Tran)** | 4,000 R/C elements, 500 time steps | 244.81 | 238.00 | **32.218** | **27.836** | `0.00e0` | **7.6×** | **8.6×** |
| **3. 64-Diode Nonlinear Array (201-Pt DC Sweep)** | 64 Diodes + 128 Resistors, 201 sweep points | 8.71 | 5.00 | **2.898** | **2.664** | `2.15e-14` | **3.0×** | **1.9×** |
| **4. 100-Stage CMOS Inverter Chain (Tran)** | 200 NMOS/PMOS + 100 Caps, 400 time steps | 42.14 | 39.00 | **16.265** | **15.857** | `3.31e-12` | **2.6×** | **2.5×** |
| **5. 200-Param Sensitivity (Adjoint vs. FD)** | 200 Resistor Gradients (1 Adjoint Solve vs. 400 FD) | 2,167.17 | 800.00 | **0.345** | **0.081** | `1.50e-11` | **6,284.9×** | **9,867.0×** |
| **6. 128-Corner PVT Batch (Rayon Parallel)** | 128 Nonlinear 32-Diode Corners in Parallel | 686.08 | 256.00 | **2.136** | **1.940** | `0.00e0` | **321.1×** | **131.9×** |

<!-- rustspice-ngspice-benchmark:end -->

Every benchmark verifies numerical agreement against `ngspice-47` (`set numdgt=12`, `reltol=1e-9`, `vntol=1e-12`, `abstol=1e-14`) or central finite differences (`Max |ΔV| ≤ 3.31e-12 V`).

- **Zero-indirection symbolic LU & Cranelift JIT (`rustspice-backend-cpu::jit`)**: compiles circuit-specific sparse LU factorization and triangular solves directly into unrolled native machine code, eliminating sparse row/column index lookups inside the Newton-Raphson loop.
- **Conflict-free static gather table (`CscGatherTable`)**: pre-stamps linear conductances once during compilation and gathers only active dynamic/nonlinear entries via contiguous offsets with zero atomic contention or binary search.
- **Constant-step LU caching & secant predictor (`rustspice-solver`)**: reuses factored `L`/`U` matrices across fixed-step linear transient intervals and extrapolates nonlinear initial guesses across `.dc` sweeps and `.tran` steps.
- **Discrete adjoint sensitivity (`AdjointSensitivityEngine`)**: computes exact gradients with respect to `P` parameters in a single transposed solve (`J^T λ = ∂L/∂V`) rather than `2P` perturbed simulations.

[Complete benchmark report](docs/benchmark_report.md) · [Architecture guide](docs/architecture.md).

## Quick start

Install [Rust with rustup](https://rustup.rs) and Git. The CPU flow needs no GPU or external solver libraries. Run these commands in one POSIX shell:

```sh
git clone https://github.com/lab-emi/RustSpice.git
cd RustSpice
cargo build --release --locked -p rustspice-cli

# 1. Run DC operating point and transient simulation on a 5-stage CMOS inverter chain
./target/release/rustspice run examples/cmos_ring_oscillator.sp --backend cpu

# 2. Compute discrete adjoint parameter sensitivities dV(out)/d(theta)
./target/release/rustspice adjoint examples/cmos_ring_oscillator.sp --target-node out

# 3. Emit auto-differentiated, CSE-optimized device kernel code (msl, wgsl, cuda-tile, cuda-ptx, rust)
./target/release/rustspice codegen --model mosfet-ekv --target msl

# 4. Run the head-to-head accuracy and speed benchmark suite against local ngspice
./target/release/rustspice bench-ngspice --output docs/benchmark_report.md
```

## Built for CLI, heterogeneous compute and AI4EDA workflows

| Command / Interface | Purpose |
| :--- | :--- |
| `rustspice run <netlist> --backend <backend>` | Execute `.op`, `.dc`, and `.tran` analyses on `cpu`, `metal`, `cuda-tile`, `cuda-simt`, or `wasm` |
| `rustspice adjoint <netlist> --target-node <node>` | Compute exact gradients `∂V(node)/∂θ` across all circuit parameters in one transposed LU solve |
| `rustspice codegen --model <m> --target <t>` | Lower symbolic SSA device equations into optimized `Rust`, `MSL (df64)`, `WGSL (df64)`, `CUDA Tile`, or `CUDA PTX` kernels |
| `rustspice bench-ngspice --output <path>` | Run black-box accuracy and wall-time benchmarks against an external `ngspice` executable |
| `rustspice_py.PySimulator` (`DLPack`) | Zero-copy `DLManagedTensor` gradient and waveform export into `PyTorch` and Apple `MLX` (`examples/ai4eda_opamp_sizing.py`) |

## Compiler-driven simulation architecture

- **Single physics equation frontend (`#[spice_device]` & `rustspice-ir`)**: author forward device equations `I(V, θ)` and `Q(V, θ)` once; forward-mode and reverse-mode automatic differentiation synthesize analytical Jacobians (`∂I/∂V`, `∂Q/∂V`) and parameter pullbacks (`∂I/∂θ`), followed by constant folding, algebraic simplification, common subexpression elimination (CSE), and dead code elimination (DCE).
- **64-byte aligned SoA & zero-allocation hot loops (`rustspice-core`)**: device instances are packed into cache-aligned `DeviceSoaBuffer` arrays with pre-allocated state vectors and static `CscGatherTable` mappings. Zero heap allocations occur during DC sweeps, Newton-Raphson iterations, or transient stepping.
- **Multi-regime linear and nonlinear solvers (`rustspice-solver`)**: selects automatically between Cranelift JIT unrolled sparse LU, static-pivot symbolic-once LU with factorization caching, `faer` supernodal LU, and preconditioned conjugate gradient (PCG), guarded by `pnjlim`/`fetlim` damping and `Gmin`/source stepping homotopy.
- **Heterogeneous execution backends**:
  - **CPU (`rustspice-backend-cpu`)**: `pulp` runtime SIMD (`AVX-512` / `AVX2` / `NEON`), `rayon` parallel PVT batching, and `Cranelift` native machine-code JIT.
  - **Apple Silicon Metal (`rustspice-backend-metal`)**: unified memory (`MTLResourceStorageModeShared`) zero-copy buffers with `fma`-accelerated `df64` (`float2`) shaders and `f64` iterative refinement.
  - **NVIDIA CUDA dual-track (`rustspice-backend-cuda-tile` & `rustspice-backend-cuda-simt`)**: tile-based SoA batch evaluation and PTX SIMT gather-stamping + PCG.
  - **WebAssembly & WebGPU (`rustspice-backend-wasm`)**: hybrid WASM `simd128` and WGSL `df64` compute pipeline.

## Documentation

The [documentation site](https://lab-emi.github.io/RustEDA/modules/rustspice/) integrates into the unified RustEDA documentation. All source pages also live in this repository.

| Start here | Go deeper |
| :--- | :--- |
| [Documentation home](https://lab-emi.github.io/RustEDA/modules/rustspice/) | [Architecture and crate boundaries](docs/architecture.md) |
| [Benchmark report vs. ngspice-47](docs/benchmark_report.md) | [AI4EDA differentiable sizing example](examples/ai4eda_opamp_sizing.py) |
| [CMOS ring oscillator deck](examples/cmos_ring_oscillator.sp) | [RustEDA documentation](https://lab-emi.github.io/RustEDA/) |

## Development and contributing

RustSpice is clean-room Rust. Reference simulators (`ngspice`) interoperate strictly as black-box external executables over standard SPICE netlists in differential accuracy and performance harnesses.

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test --workspace --locked
```

Licensed under [Apache-2.0](LICENSE).
