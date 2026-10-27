//! Symbolic Static Single Assignment (SSA) Equation IR, Automatic Differentiation,
//! Compiler Optimizations (Constant Folding, Algebraic Simplification, CSE, DCE),
//! and Heterogeneous Hardware Code Generators (CPU SIMD, Apple Metal MSL `df64`,
//! WebGPU WGSL `df64`, and NVIDIA CUDA Tile/PTX).

pub mod autodiff;
pub mod codegen;
pub mod models;
pub mod opt;
pub mod ssa;

pub use autodiff::{AdjointGradMap, DiffTarget};
pub use codegen::{
    emit_cuda_ptx_kernel, emit_cuda_tile_kernel, emit_msl_df64_kernel, emit_rust_scalar_fn,
    emit_wgsl_df64_kernel,
};
pub use models::{build_diode_ir, build_mosfet_ekv_ir, build_mosfet_level1_ir};
pub use opt::{optimize_graph, OptimizationStats};
pub use ssa::{BinaryOp, ExprId, IrGraph, IrNode, NamedOutput, UnaryOp};
