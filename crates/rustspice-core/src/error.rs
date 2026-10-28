//! Error types and result aliases for RustSpice simulation operations.

use thiserror::Error;

/// Unified error type across parsing, compilation, hardware backends, and numerical solvers.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SimError {
    #[error("Netlist parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Topology error: {0}")]
    TopologyError(String),

    #[error("Singular Jacobian matrix encountered at column/row {index}")]
    SingularMatrix { index: usize },

    #[error("Newton-Raphson failed to converge after {iterations} iterations (residual = {residual:.3e})")]
    ConvergenceFailure { iterations: usize, residual: f64 },

    /// A converged iterate lies outside the range in which a device model is evaluated exactly
    /// (for example a diode junction whose exponent argument exceeds the f64 overflow guard).
    #[error("Device model range exceeded: {0}")]
    DeviceRangeExceeded(String),

    #[error("Transient simulation failed at t = {time:.3e}s: {reason}")]
    TransientFailure { time: f64, reason: String },

    #[error("Hardware backend error ({backend}): {message}")]
    BackendError { backend: String, message: String },

    #[error("OSDI / Verilog-A model error: {0}")]
    OsdiError(String),

    /// A work or memory budget (see [`crate::limits::ResourceLimits`]) was exceeded; raised BEFORE the
    /// allocation or work it protects.
    #[error("Resource limit exceeded ({resource}): requested {requested}, limit {limit}. {hint}")]
    ResourceLimit {
        resource: &'static str,
        requested: String,
        limit: String,
        hint: String,
    },

    /// A linear solve produced an infinite or NaN unknown (overflow): the result is not representable.
    #[error("Non-finite value (overflow or NaN) in MNA unknown {unknown}: the solution is not representable in f64")]
    NonFiniteSolution { unknown: usize },
}

/// Standard result alias returning [`SimError`].
pub type SimResult<T> = Result<T, SimError>;
