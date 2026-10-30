<p align="center">
  <img src="docs/assets/rustspice-wordmark.svg" alt="RustSpice" width="360">
</p>

<h1 align="center">Circuit simulation for the next design iteration.</h1>

<p align="center">
  <a href="https://lab-emi.github.io/RustEDA/modules/rustspice/">Documentation</a> ·
  <a href="https://github.com/lab-emi/RustEDA">RustEDA</a> ·
  <a href="https://lab-emi.github.io/RustEDA/benchmarks/">Benchmarks</a> ·
  <a href="LICENSE">Apache-2.0</a>
</p>

<picture>
  <source media="(prefers-reduced-motion: reduce)" srcset="docs/assets/rustspice-banner.svg">
  <img src="docs/assets/rustspice-banner.gif" alt="RustSpice simulates SPICE netlists with Structure-of-Arrays device kernels, topology-specialized sparse solvers and discrete adjoint sensitivities" width="1200">
</picture>

RustSpice simulates transistor-level SPICE netlists (`.op`, `.dc`, `.tran`) and computes discrete adjoint parameter sensitivities with Structure-of-Arrays (SoA) device evaluation, auto-differentiated model kernels and topology-specialized sparse linear solvers.

Part of **[RustEDA](https://github.com/lab-emi/RustEDA)**, infrastructure for chip-design recursive self-improvement. Native tools shorten the work between inference steps; verified results guide the next iteration.

## Benchmarks

RustEDA publishes one benchmark: a matched Ibex RTL-to-GDS comparison with LibreLane on IHP SG13G2 / EZ130. See the [RustEDA benchmarks](https://lab-emi.github.io/RustEDA/benchmarks/). RustSpice publishes no performance comparison; the [benchmark method](docs/benchmark_report.md) describes how to measure one against `ngspice` on your own host.

## Differential verification against ngspice

`rustspice bench-suite` runs 951 cases against ngspice-47 as a black-box executable: **857 pass**, **16 documented RustSpice gaps**, **77 informational**, **1 unverified** and **0 failing**, plus 400/400 random fuzz decks passing and 200/200 hostile mutants handled without a panic. See [`benchmarks/README.md`](benchmarks/README.md).

**Batched DC operating points on an NVIDIA GPU** (`rustspice bench-batch`, [method](docs/gpu_batch_report.md)): one circuit at many corners, one corner per CUDA thread in FP64, with a Newton / LU kernel written in Rust and compiled to PTX by [cuda-oxide](https://github.com/NVIDIA/cuda-rust); the same source, built for the CPU with rayon, is the reference it is tested against. The kernel covers plain diodes and level-1 MOSFETs with their bulk junctions. A corner the kernel does not converge is re-solved by the CPU solver. In the last recorded run, the GPU agreed with the CPU build to `5.6e-16` (bit for bit on the MOSFET circuits) and with the CPU solver to `1.8e-9` (relative, inside its Newton tolerance), and no corner needed a CPU re-solve. Creating the CUDA context and JIT-compiling the PTX is a one-time cost per process, so small batches can run faster on the CPU. MOS levels above 1, diodes with breakdown, series resistance or a knee current, polynomial sources, `.nodeset` and linear circuits still run on the CPU.

## Simulator features

- **Three-tier adaptive sparse linear solver (`rustspice-solver::linear`)**: combines symmetric minimum-degree ordering, diagonal-preferring threshold Markowitz pivoting (`0.1`) and factor-growth monitoring on the unrolled symbolic / Cranelift JIT tier with cached symbolic-once `faer` sparse LU (`FaerLuCache`), dense partial-pivoting LU for small ill-conditioned topologies, and Jacobi-preconditioned conjugate gradient (PCG).
- **Conflict-free static gather table (`CscGatherTable`)**: pre-stamps linear conductances once during compilation and gathers only active dynamic/nonlinear entries via contiguous offsets with zero atomic contention or binary search.
- **Adaptive LTE transient integration, quantized LU caching & homotopy (`rustspice-solver`)**: controls step size via divided-difference local truncation error (LTE), lands exactly on waveform breakpoints (`PULSE`, `SIN`, `EXP`, `PWL`) with L-stable `BE -> BE -> BDF-2 -> Trapezoidal` restarts sharing companion scaling (`w = phi`) across a quantized `hmax / 2^k` `FactorCache`, supports `.ic`, `uic`, `tstart`, `tmax`, `.nodeset`, and extended `.dc` sweeps (source, resistor, temperature, 2-variable nested), and pairs `pnjlim`/`fetlim` step-delta checks with adaptive `Gmin` and source-stepping homotopy.
- **Strict SPICE reader, expression/include engine & resource budgets (`rustspice-core`)**: resolves `.param` expressions, `.func` definitions, `.subckt ... PARAMS:` overrides, `.include`/`.lib` sections, and `.temp`/`.options` temperature scaling (`m=`, `tc1=`, `tc2=`), refuses unsupported directives or model levels via structured `Diagnostic` errors/warnings, and enforces configurable `ResourceLimits` (`--max-elements`, `--max-nodes`, `--max-subckt-depth`, `--max-sweep-points`, `--max-tran-steps`, `--max-newton-iters`).
- **Device and element coverage (`rustspice-core::ext`)**: diodes with series resistance, breakdown (BV/IBV), high injection (IKF), recombination current (ISR/NR), transit time and graded depletion capacitance; level-1 MOSFETs with process-derived KP/VTO/GAMMA/PHI (TOX, UO, NSUB, NSS, TPG), LD, RD/RS/RSH, bulk junctions and Meyer gate capacitances; E/F/H controlled sources, `POLY(1)` E/G sources, coupled inductors (K) and semiconductor `.model R`/`.model C`. BJT, JFET, MESFET, B, S, W, T, O, U and MOS LEVEL > 1 are refused with an error.
- **Discrete adjoint sensitivity (`AdjointSensitivityEngine`)**: computes exact gradients with respect to `P` parameters in a single transposed solve (`J^T λ = ∂L/∂V`) rather than `2P` perturbed simulations.

[Benchmark method](docs/benchmark_report.md) · [Architecture guide](docs/architecture.md).

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
./target/release/rustspice bench-ngspice --output target/bench-ngspice.md
```

## Built for CLI, heterogeneous compute and AI4EDA workflows

| Command / Interface | Purpose |
| :--- | :--- |
| `rustspice run <netlist> --backend <backend>` | Execute `.op`, `.dc`, and `.tran` analyses on `cpu`, `metal`, `cuda-tile`, `cuda-simt`, or `wasm` |
| `rustspice adjoint <netlist> --target-node <node>` | Compute exact gradients `∂V(node)/∂θ` across all circuit parameters in one transposed LU solve |
| `rustspice codegen --model <m> --target <t>` | Lower symbolic SSA device equations into optimized `Rust`, `MSL (df64)`, `WGSL (df64)`, `CUDA Tile`, or `CUDA PTX` kernels |
| `rustspice bench-ngspice --output <path>` | Run black-box accuracy and wall-time benchmarks against an external `ngspice` executable |
| `rustspice bench-batch --output <path>` | Time batched DC operating points on the CPU batch API, the batch kernel's CPU build and an NVIDIA GPU |
| `rustspice run <netlist> --internal-points` | Also write the accepted internal time steps between the `k * tstep` rows of a transient rawfile |
| `rustspice_py.PySimulator` (`DLPack`) | Zero-copy `DLManagedTensor` gradient and waveform export into `PyTorch` and Apple `MLX` (`examples/ai4eda_opamp_sizing.py`) |

## Compiler-driven simulation architecture

- **Single physics equation frontend (`#[spice_device]` & `rustspice-ir`)**: author forward device equations `I(V, θ)` and `Q(V, θ)` once; forward-mode and reverse-mode automatic differentiation synthesize analytical Jacobians (`∂I/∂V`, `∂Q/∂V`) and parameter pullbacks (`∂I/∂θ`), followed by constant folding, algebraic simplification, common subexpression elimination (CSE), and dead code elimination (DCE).
- **64-byte aligned SoA & zero-allocation hot loops (`rustspice-core`)**: device instances are packed into cache-aligned `DeviceSoaBuffer` arrays with pre-allocated state vectors and static `CscGatherTable` mappings. Zero heap allocations occur during DC sweeps, Newton-Raphson iterations, or transient stepping.
- **Multi-regime linear and nonlinear solvers (`rustspice-solver`)**: selects automatically between Cranelift JIT unrolled sparse LU, static-pivot symbolic-once LU with factorization caching, `faer` supernodal LU, and preconditioned conjugate gradient (PCG), guarded by `pnjlim`/`fetlim` damping and `Gmin`/source stepping homotopy.
- **Heterogeneous execution backends**:
  - **CPU (`rustspice-backend-cpu`)**: `pulp` runtime SIMD (`AVX-512` / `AVX2` / `NEON`), `rayon` parallel PVT batching, and `Cranelift` native machine-code JIT.
  - **Apple Silicon Metal (`rustspice-backend-metal`)**: unified memory (`MTLResourceStorageModeShared`) zero-copy buffers with `fma`-accelerated `df64` (`float2`) shaders and `f64` iterative refinement.
  - **NVIDIA CUDA dual-track (`rustspice-backend-cuda-tile` & `rustspice-backend-cuda-simt`)**: tile-based SoA batch evaluation, PTX SIMT gather-stamping + PCG, and batched FP64 DC operating points (one corner per CUDA thread) whose kernel is written in Rust, compiled to PTX by [cuda-oxide](https://github.com/NVIDIA/cuda-rust) and differentially tested against its own rayon `cpu_ref` build. The CUDA driver is loaded at run time, so the build needs no CUDA toolkit.
  - **WebAssembly & WebGPU (`rustspice-backend-wasm`)**: hybrid WASM `simd128` and WGSL `df64` compute pipeline.

## Documentation

The [documentation site](https://lab-emi.github.io/RustEDA/modules/rustspice/) integrates into the unified RustEDA documentation. All source pages also live in this repository.

| Start here | Go deeper |
| :--- | :--- |
| [Documentation home](https://lab-emi.github.io/RustEDA/modules/rustspice/) | [Architecture and crate boundaries](docs/architecture.md) |
| [Benchmark method vs. ngspice-47](docs/benchmark_report.md) | [AI4EDA differentiable sizing example](examples/ai4eda_opamp_sizing.py) |
| [CMOS ring oscillator deck](examples/cmos_ring_oscillator.sp) | [RustEDA documentation](https://lab-emi.github.io/RustEDA/) |

## Development and contributing

RustSpice is clean-room Rust. Reference simulators (`ngspice`) interoperate strictly as black-box external executables over standard SPICE netlists in differential accuracy and performance harnesses.

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test --workspace --locked
```

Licensed under [Apache-2.0](LICENSE).
