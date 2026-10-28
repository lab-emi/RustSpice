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

**Host: 13th Gen Intel(R) Core(TM) i9-13900HX (32 hardware threads) · Reference: `ngspice-47` (SPARSE 1.3 by default, KLU with `.options klu`; every row compares with the faster of the two).**

Both tools run as subprocesses (`rustspice run --raw`, `ngspice -b -r`) with the rawfile on tmpfs, the single-core benchmarks pinned to one idle core: one warm-up, nine interleaved repetitions, end to end (process start-up, parsing, topology compilation, symbolic factorisation, device stamping, the solve and the output). Accuracy is the maximum error over the full waveform or sweep against an independent oracle where one exists, otherwise against a tightly converged ngspice (`reltol=1e-9`, `vntol=1e-12`, `abstol=1e-15`).

<!-- rustspice-ngspice-benchmark:start -->

| Benchmark | Circuit scale | `ngspice-47`<br>end to end ↓ | `RustSpice`<br>end to end ↓ | Speedup ↑ | Max error: `RustSpice` / `ngspice` default ↓ |
| :--- | :--- | ---: | ---: | ---: | ---: |
| **1. Resistor ladder (DC OP)** | 10,000 resistors, 5,002 MNA unknowns (DC operating point) | 23.66 ms | 10.17 ms | **2.33×** | `1.91e-13` / `1.93e-13` V |
| **2. RC line (transient)** | 4,000 R/C elements, `.tran 10p 5n` (500 steps) | 104 ms | 29.29 ms | **3.56×** (at **8.2× lower error** than ngspice default) | `2.01e-4` / `1.65e-3` of swing (vs tight ngspice) |
| **3. Diode array (DC sweep)** | 64 diodes + 128 resistors, 201-point DC sweep | 4.82 ms | 2.55 ms | **1.89×** | `5.89e-11` / `5.44e-5` V |
| **4. CMOS inverter chain (transient)** | 200 MOSFETs + 100 capacitors, `.tran 5p 2n` (400 steps) | 31.65 ms | 28.34 ms | **1.12×** (at **5.2× lower error** than ngspice default) | `7.40e-2` / `3.86e-1` of swing (vs tight ngspice) |
| **5. Sensitivity (adjoint vs. ngspice `.sens`)** | 200 resistor gradients (one adjoint solve) | 11.16 ms | 1.08 ms | **10.3×** vs `.sens` (**333×** vs FD loop) | `9.33e-15` / `1.00e-6` (normalised) |
| **6. PVT batch (128 corners)** | 128 PVT corners of a 32-diode nonlinear net (Rayon) | 3.69 ms | 3.56 ms | **1.03×** (parallel workload; **2.71×** via `.dc` sweep) | `2.47e-11` / `1.63e-5` V |

<!-- rustspice-ngspice-benchmark:end -->

Speedup = `ngspice-47` / `RustSpice`, median of 9 interleaved repetitions against the faster of ngspice's SPARSE 1.3 and KLU solvers; both tools run as subprocesses with rawfile output on tmpfs, the single-core benchmarks pinned to one idle core (the parallel batch, benchmark 6, is not pinned). The previously published 5.3×, 7.6×, 3.0×, 2.6×, 6,285× and 321× came from a harness with known defects (1 ms-quantised solver times, a cold process compared with a warm in-process call, hard-coded zero errors, single runs multiplied by 128 or 400, a transient compared at t = 0 only) and are withdrawn; the report lists each one with its replacement.

- Benchmark 2: restarting fast slope corners with one short backward-Euler step, and a jump detector that no longer takes a steep continuous edge for a jump, reduced RustSpice's waveform error by 23× (from `4.61e-3` to `2.01e-4` of swing, 8.2× below ngspice default's `1.65e-3`); on the step ladder `b2s_rcline_500` (`tstep` 10 ps..1 ps) RustSpice is both 3.5×–4.2× faster and 2.7×–15× more accurate than ngspice default.
- Benchmark 4: a quadratic predictor, contraction-rate Newton acceptance and a jagged Jacobian gather cut RustSpice's analysis time from 47 ms to 26 ms (with the level-1 bulk junctions and Meyer capacitances added since); it is now faster than ngspice end to end at 5.2× lower waveform error (100th-stage 50% crossing shift `1.60 ps` vs ngspice default's `9.10 ps`).
- Benchmark 5 compares one adjoint solve with the best ngspice alternative (`.sens`, which computes 24 sensitivities per requested gradient); against the finite-difference loop that delivers only the 200 requested gradients the ratio is 333×.
- Benchmark 6 is a parallel workload (32 threads vs ngspice's 1-thread `.dc` sweep; via `rustspice-dc-sweep` the 1-thread ratio is 2.71×, against `ngspice-control-loop-sparse` 5.97×, and against `ngspice-par-launches` 9.66×).

Beyond these six circuits, `rustspice bench-suite` runs 951 cases against ngspice-47 (**857 pass**, **16 documented RustSpice gaps**, **77 informational**, **1 unverified** without timing, **0 failing**, plus 400/400 random fuzz decks passing and 200/200 hostile mutants handled without a panic) and times 110 of them: RustSpice is faster end to end in **101 of 110**, with a geometric-mean speedup of **2.51×**. All 2-D and 3-D linear meshes, shuffled ladders and LC ladders are faster than ngspice; 9 nonlinear 2-D MOS/SRAM/diode array and mesh cases remain 1.03×–24.4× slower than ngspice-KLU. See [`benchmarks/README.md`](benchmarks/README.md).

- **Three-tier adaptive sparse linear solver (`rustspice-solver::linear`)**: combines symmetric minimum-degree ordering, diagonal-preferring threshold Markowitz pivoting (`0.1`) and factor-growth monitoring on the unrolled symbolic / Cranelift JIT tier with cached symbolic-once `faer` sparse LU (`FaerLuCache`), dense partial-pivoting LU for small ill-conditioned topologies, and Jacobi-preconditioned conjugate gradient (PCG).
- **Conflict-free static gather table (`CscGatherTable`)**: pre-stamps linear conductances once during compilation and gathers only active dynamic/nonlinear entries via contiguous offsets with zero atomic contention or binary search.
- **Adaptive LTE transient integration, quantized LU caching & homotopy (`rustspice-solver`)**: controls step size via divided-difference local truncation error (LTE), lands exactly on waveform breakpoints (`PULSE`, `SIN`, `EXP`, `PWL`) with L-stable `BE -> BE -> BDF-2 -> Trapezoidal` restarts sharing companion scaling (`w = phi`) across a quantized `hmax / 2^k` `FactorCache`, supports `.ic`, `uic`, `tstart`, `tmax`, `.nodeset`, and extended `.dc` sweeps (source, resistor, temperature, 2-variable nested), and pairs `pnjlim`/`fetlim` step-delta checks with adaptive `Gmin` and source-stepping homotopy.
- **Strict SPICE reader, expression/include engine & resource budgets (`rustspice-core`)**: resolves `.param` expressions, `.func` definitions, `.subckt ... PARAMS:` overrides, `.include`/`.lib` sections, and `.temp`/`.options` temperature scaling (`m=`, `tc1=`, `tc2=`), refuses unsupported directives or model levels via structured `Diagnostic` errors/warnings, and enforces configurable `ResourceLimits` (`--max-elements`, `--max-nodes`, `--max-subckt-depth`, `--max-sweep-points`, `--max-tran-steps`, `--max-newton-iters`).
- **Device and element coverage (`rustspice-core::ext`)**: diodes with series resistance, breakdown (BV/IBV), high injection (IKF), recombination current (ISR/NR), transit time and graded depletion capacitance; level-1 MOSFETs with process-derived KP/VTO/GAMMA/PHI (TOX, UO, NSUB, NSS, TPG), LD, RD/RS/RSH, bulk junctions and Meyer gate capacitances; E/F/H controlled sources, `POLY(1)` E/G sources, coupled inductors (K) and semiconductor `.model R`/`.model C`. BJT, JFET, MESFET, B, S, W, T, O, U and MOS LEVEL > 1 are refused with an error.
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
