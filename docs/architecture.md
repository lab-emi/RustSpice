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
   - **Multi-Regime Linear & Nonlinear Solver (`rustspice-solver`)**: adapts automatically between Cranelift JIT unrolled LU, static-pivot symbolic-once sparse LU with constant-step factorization caching, `faer` supernodal LU, and preconditioned conjugate gradient (PCG), paired with secant trajectory prediction, Shamanskii Jacobian reuse, and `pnjlim`/`fetlim` voltage damping.
   - **Apple Silicon Metal (`rustspice-backend-metal`)**: unified memory (`MTLResourceStorageModeShared`) zero-copy buffers with hardware `fma`-accelerated `df64` double-float (`float2`, ~48-bit significand) shaders and CPU `f64` iterative refinement.
   - **NVIDIA CUDA Dual-Track (`rustspice-backend-cuda-tile` & `rustspice-backend-cuda-simt`)**: tile-based SoA batch device evaluation (`cutile-rs` track) and PTX SIMT conflict-free gather-stamping + PCG (`cuda-oxide` track).
   - **WebAssembly & WebGPU (`rustspice-backend-wasm`)**: hybrid WASM `simd128` and `wgpu` WGSL `df64` compute shader pipeline.
   - **AI4EDA Bridge (`rustspice-py`)**: zero-copy `DLPack` (`DLManagedTensor`) capsule export for `PyTorch` and Apple `MLX`.

## Crate organization

| Crate | Role |
| :--- | :--- |
| `rustspice-core` | Zero-copy SPICE netlist lexer/parser, 64-byte aligned `DeviceSoaBuffer`, `CscMatrix`, `CscGatherTable`, `df64` precision types, and OSDI dynamic model loader |
| `rustspice-ir` | Symbolic SSA equation IR, forward/reverse automatic differentiation, CSE/DCE optimization, and multi-target shader/kernel codegen (`Rust`, `MSL`, `WGSL`, `CUDA Tile`, `CUDA PTX`) |
| `rustspice-macros` | Procedural macro `#[spice_device]` for compile-time device Jacobian and adjoint code generation |
| `rustspice-backend-cpu` | Branchless SIMD (`pulp`), multi-core batch runner (`rayon`), and `Cranelift` topology-specialized sparse LU machine-code JIT |
| `rustspice-backend-metal` | Apple Silicon UMA zero-copy backend with `df64` MSL kernels and mixed-precision iterative refinement |
| `rustspice-backend-cuda-tile` | NVIDIA tile-based batch device evaluation backend (`cutile-rs` programming model) |
| `rustspice-backend-cuda-simt` | NVIDIA PTX SIMT conflict-free gather-stamping and GPU PCG solver backend |
| `rustspice-backend-wasm` | Browser/cloud-native WASM `simd128` + WebGPU WGSL `df64` backend |
| `rustspice-solver` | Multi-regime sparse linear solver, damped Newton-Raphson (`.op`, `.dc`), adaptive BDF-1/2 & trapezoidal `.tran`, and discrete adjoint sensitivity engine |
| `rustspice-py` | `PyO3` bindings and zero-copy `DLPack` tensor bridge to `PyTorch` and Apple `MLX` |
| `rustspice-cli` | `rustspice` command-line interface and automated `ngspice` head-to-head verification/benchmark harness |
