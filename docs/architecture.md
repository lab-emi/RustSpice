# Architecture and Engineering Guide

## Compiler-driven heterogeneous architecture

RustSpice replaces legacy pointer-linked Array-of-Structures (AoS) SPICE architectures with a three-stage compiler-driven pipeline:

1. **Single Physics Equation Frontend (`#[spice_device]` & `rustspice-ir`)**:
   - Device equations are authored once as pure forward mathematical expressions for terminal current `I(V, θ)` and charge `Q(V, θ)`.
   - `rustspice-ir` lowers expressions into Static Single Assignment (SSA) graphs, synthesizes analytical Jacobians (`∂I/∂V`, `∂Q/∂V`) via forward-mode automatic differentiation and parameter pullbacks (`∂I/∂θ`) via reverse-mode automatic differentiation, and runs constant folding, algebraic simplification, common subexpression elimination (CSE), and dead code elimination (DCE).
2. **Pre-Compiled Conflict-Free Topology (`rustspice-core::soa` & `sparse`)**:
   - Devices are packed into 64-byte cache-line-aligned Structure-of-Arrays buffers (`DeviceSoaBuffer`).
   - Static linear conductances are pre-stamped once into `static_linear_csc_values`.
   - Dynamic and nonlinear terms are mapped via `CscGatherTable::active_nnz` and `active_rhs`, eliminating atomic contention and binary searches during Newton-Raphson and transient iterations.
   - All working vectors (`voltages`, `rhs`, `delta`, `prev_voltages`, `charge_history`) and sparse matrix buffers are pre-allocated during `Simulator::from_spice()`, guaranteeing zero heap allocations inside hot solve loops.
3. **Multi-Regime Solvers and Hardware Backends**:
   - **CPU (`rustspice-backend-cpu`)**: `pulp` runtime SIMD (`AVX-512` / `AVX2` / `NEON`), `rayon` work-stealing batch execution, and `Cranelift` native machine-code JIT compilation (`CraneliftTopologyJit`) for unrolled zero-indirection sparse LU factorization and forward/backward substitution.
   - **Multi-Regime Linear & Nonlinear Solver (`rustspice-solver`)**: adapts automatically between Cranelift JIT unrolled LU, static-pivot symbolic-once sparse LU with constant-step factorization caching (`FactorCache` on the `hmax / 2^k` ladder), `faer` supernodal LU (`FaerLuCache`), dense partial-pivot LU, and preconditioned conjugate gradient (PCG), paired with quadratic/secant trajectory prediction, contraction-rate Newton acceptance, Shamanskii Jacobian reuse, `pnjlim`/`fetlim` voltage damping, adaptive Gmin/source-stepping homotopy, and divided-difference LTE adaptive step control with `BE -> BE -> BDF-2 -> Trapezoidal` restarts at waveform breakpoints and fast slope corners.
   - **Apple Silicon Metal (`rustspice-backend-metal`)**: unified memory (`MTLResourceStorageModeShared`) zero-copy buffers with hardware `fma`-accelerated `df64` double-float (`float2`, ~48-bit significand) shaders and CPU `f64` iterative refinement.
   - **NVIDIA CUDA Dual-Track (`rustspice-backend-cuda-tile` & `rustspice-backend-cuda-simt`)**: tile-based SoA batch device evaluation (`cutile-rs` track), PTX SIMT conflict-free gather-stamping + PCG, and a single-launch FP64 batched DC Newton-Raphson + sparse LU kernel (`kernels/batch_newton.rs` compiled via `rustc -Z codegen-backend=cuda_oxide` to `nvptx64-nvidia-cuda`, paired with a deterministic `solve_cpu_ref` twin).
   - **WebAssembly & WebGPU (`rustspice-backend-wasm`)**: hybrid WASM `simd128` and `wgpu` WGSL `df64` compute shader pipeline.
   - **AI4EDA Bridge (`rustspice-py`)**: zero-copy `DLPack` (`DLManagedTensor`) capsule export for `PyTorch` and Apple `MLX`.

## Crate organization

| Crate | Role |
| :--- | :--- |
| `rustspice-core` | Strict SPICE netlist reader (`.param`, `.func`, `.include`/`.lib`, `.subckt PARAMS`, `.temp`, waveforms & breakpoints), 64-byte aligned `DeviceSoaBuffer`, extended device physics (`ext.rs`: diode `RS`/`BV`/`IBV`/`IKF`/`ISR`/`TT`/`CJO`, level-1 MOSFET `UO`/`TOX`/`NSUB`/`LD`/`RD`/`RS` + bulk junctions + Meyer capacitances, `E`/`F`/`H`, `POLY(1)`, mutual inductance `K`, `.model R`/`C`), `CscMatrix`, `CscGatherTable`, `df64` precision types, and OSDI dynamic model loader |
| `rustspice-ir` | Symbolic SSA equation IR, forward/reverse automatic differentiation, CSE/DCE optimization, and multi-target shader/kernel codegen (`Rust`, `MSL`, `WGSL`, `CUDA Tile`, `CUDA PTX`) |
| `rustspice-macros` | Procedural macro `#[spice_device]` for compile-time device Jacobian and adjoint code generation |
| `rustspice-backend-cpu` | Branchless SIMD (`pulp`), multi-core batch runner (`rayon`), and `Cranelift` topology-specialized sparse LU machine-code JIT |
| `rustspice-backend-metal` | Apple Silicon UMA zero-copy backend with `df64` MSL kernels and mixed-precision iterative refinement |
| `rustspice-backend-cuda-tile` | NVIDIA tile-based batch device evaluation backend (`cutile-rs` programming model) |
| `rustspice-backend-cuda-simt` | NVIDIA `cuda-oxide` FP64 batched DC Newton/LU kernel (`batch_newton.ptx`), PTX SIMT conflict-free gather-stamping, and GPU PCG solver backend |
| `rustspice-backend-wasm` | Browser/cloud-native WASM `simd128` + WebGPU WGSL `df64` backend |
| `rustspice-solver` | Multi-regime sparse linear solver, damped Newton-Raphson (`.op`, `.dc`), CPU/CUDA batched DC operating points (`batch_dc`), adaptive LTE BDF-1/2 & trapezoidal `.tran`, and discrete adjoint sensitivity engine |
| `rustspice-py` | `PyO3` bindings and zero-copy `DLPack` tensor bridge to `PyTorch` and Apple `MLX` |
| `rustspice-cli` | `rustspice` command-line interface and automated `ngspice` head-to-head verification/benchmark harness |
