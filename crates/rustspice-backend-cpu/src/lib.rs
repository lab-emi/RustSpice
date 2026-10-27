//! Heterogeneous CPU Execution Backend:
//! - [`CpuSimdBackend`]: Hardware SIMD (`pulp` AVX-512 / AVX2 / ARM NEON) branchless SoA device
//!   evaluation and conflict-free gather stamping.
//! - [`CraneliftTopologyJit`]: Topology-specialized native machine code JIT compiler (`cranelift-jit`)
//!   generating zero-loop, zero-indirection sparse LU factorization and triangular solve kernels.
//! - [`BatchCpuExecutor`]: Multi-threaded `rayon` batch simulator for PVT corners, Monte Carlo,
//!   and AI4EDA reinforcement learning rollouts.

pub mod batch;
pub mod jit;
pub mod simd;

pub use batch::{BatchCircuitInput, BatchCircuitOutput, BatchCpuExecutor};
pub use jit::{CraneliftTopologyJit, SymbolicLuOp, UnrolledSymbolicLu};
pub use simd::{fetlim, pnjlim, CpuSimdBackend};
