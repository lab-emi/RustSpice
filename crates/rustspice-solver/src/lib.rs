//! High-Performance Circuit Simulation Solver Core:
//! - [`linear`]: Three-tier adaptive sparse linear solver (`CraneliftTopologyJit` / `UnrolledSymbolicLu`
//!   zero-indirection engine, `faer` Symbolic-Once Supernodal LU, and Preconditioned Conjugate Gradient).
//! - [`nonlinear`]: Damped Newton-Raphson solver with `pnjlim`/`fetlim`, Secant Trajectory Predictor,
//!   Shamanskii LU reuse, Linear Fast-Path, and `Gmin`/`Source` Homotopy continuation.
//! - [`transient`]: Trapezoidal and BDF-1/2 transient time-domain integrator with Constant-$h$ LU
//!   factor caching and Local Truncation Error (LTE) step control.
//! - [`adjoint`]: Discrete Adjoint Sensitivity engine computing gradients with respect to thousands
//!   of circuit parameters in a single transposed triangular solve.
//! - [`batch_dc`]: batched DC operating points on the batch kernel (rayon `cpu_ref` or CUDA), with the CPU solver
//!   for the corners it leaves.

pub mod adjoint;
pub mod batch_dc;
pub mod linear;
pub mod nonlinear;
pub mod simulator;
pub mod transient;

pub use adjoint::{AdjointGradientResult, AdjointSensitivityEngine, ParameterGradSpec};
pub use batch_dc::{BatchDcEngine, BatchDcRun};
pub use linear::{AdaptiveSparseSolver, LinearSolverKind};
pub use nonlinear::{DcOpResult, DcSweepResult, NonlinearSolver, NrOptions};
pub use simulator::Simulator;
pub use transient::{IntegrationMethod, TranOptions, TranResult, TranSpec, TransientSolver};
