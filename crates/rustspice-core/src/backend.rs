//! Unified hardware execution backend trait for device evaluation and matrix stamping.

use crate::error::SimResult;
use crate::soa::DeviceSoaBuffer;
use crate::sparse::CscMatrix;

/// Unified hardware execution backend trait for device evaluation and matrix stamping.
pub trait ComputeBackend: Send + Sync {
    /// Returns the human-readable identifier of this hardware execution backend.
    fn backend_name(&self) -> &'static str {
        "generic-backend"
    }

    /// Evaluates device equations (currents, charges, and conductances) across all
    /// instances stored in Structure-of-Arrays (SoA) layout.
    fn evaluate_devices(&self, state: &mut DeviceSoaBuffer, voltages: &[f64]) -> SimResult<()>;

    /// Assembles device derivatives into the global sparse Jacobian matrix and RHS vector
    /// using pre-compiled conflict-free gather indices.
    fn stamp_jacobian(
        &self,
        state: &DeviceSoaBuffer,
        jacobian: &mut CscMatrix<f64>,
        rhs: &mut [f64],
    ) -> SimResult<()>;
}
