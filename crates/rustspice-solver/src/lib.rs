//! High-Performance Circuit Simulation Solver Core:
//! - [`linear`]: Three-tier adaptive sparse linear solver (`CraneliftTopologyJit` / `UnrolledSymbolicLu`
//!   zero-indirection engine, `faer` Symbolic-Once Supernodal LU, and Preconditioned Conjugate Gradient).
//! - [`nonlinear`]: Damped Newton-Raphson solver with `pnjlim`/`fetlim`, Secant Trajectory Predictor,
//!   Shamanskii LU reuse, Linear Fast-Path, and `Gmin`/`Source` Homotopy continuation.
//! - [`transient`]: Trapezoidal and BDF-1/2 transient time-domain integrator with Constant-$h$ LU
//!   factor caching and Local Truncation Error (LTE) step control.
//! - [`adjoint`]: Discrete Adjoint Sensitivity engine computing gradients with respect to thousands
//!   of circuit parameters in a single transposed triangular solve.

pub mod adjoint;
pub mod linear;
pub mod nonlinear;
pub mod simulator;
pub mod transient;

pub use adjoint::{AdjointGradientResult, AdjointSensitivityEngine, ParameterGradSpec};
pub use linear::{AdaptiveSparseSolver, LinearSolverKind};
pub use nonlinear::{DcOpResult, DcSweepResult, NonlinearSolver, NrOptions};
pub use simulator::Simulator;
pub use transient::{IntegrationMethod, TranOptions, TranResult, TransientSolver};
