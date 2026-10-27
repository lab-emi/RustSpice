//! Core data structures, numerical types (`Df64`, `Dual`), zero-copy SPICE netlist parser,
//! 64-byte aligned Structure-of-Arrays (`DeviceSoaBuffer`), conflict-free sparse matrix
//! `CscGatherTable`, hardware [`ComputeBackend`] trait, and OpenVAF/OSDI v0.3 interface.

pub mod backend;
pub mod error;
pub mod netlist;
pub mod osdi;
pub mod precision;
pub mod soa;
pub mod sparse;

pub use backend::ComputeBackend;
pub use error::{SimError, SimResult};
pub use netlist::{
    parse_netlist, parse_spice_number, AnalysisCmd, CircuitBuilder, DiodeModelParams,
    MosfetModelParams, MosfetType, Netlist, SourceWaveform,
};
pub use osdi::{OsdiDescriptor, OsdiLibrary, OsdiModelInstance};
pub use precision::{Df64, Dual};
pub use soa::{
    compile_netlist, intrinsic_diode_kernel_eval_with_derivatives,
    intrinsic_diode_kernel_msl_shader, intrinsic_diode_kernel_wgsl_shader,
    intrinsic_mosfet_kernel_eval_with_derivatives, intrinsic_mosfet_kernel_msl_shader,
    intrinsic_mosfet_kernel_wgsl_shader, AlignedVec, CompiledTopology, DeviceSoaBuffer,
};
pub use sparse::{CscGatherTable, CscMatrix, CsrMatrix, GatherTerm};
