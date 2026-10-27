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

    #[error("Transient simulation failed at t = {time:.3e}s: {reason}")]
    TransientFailure { time: f64, reason: String },

    #[error("Hardware backend error ({backend}): {message}")]
    BackendError { backend: String, message: String },

    #[error("OSDI / Verilog-A model error: {0}")]
    OsdiError(String),
}

/// Standard result alias returning [`SimError`].
pub type SimResult<T> = Result<T, SimError>;
