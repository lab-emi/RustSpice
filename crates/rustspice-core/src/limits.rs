//! Hard resource budgets for parsing, hierarchy expansion and result storage.
//!
//! Every budget is checked BEFORE the allocation or the work it protects and fails with a structured
//! [`SimError::ResourceLimit`] (clean exit code 1) instead of an allocator abort, a stack overflow or an
//! OOM kill. All defaults are finite; the `rustspice run` flags `--max-*` override them.

use serde::{Deserialize, Serialize};

use crate::error::{SimError, SimResult};

/// Default maximum `.subckt` instantiation depth (the expander is iterative, so this bounds
/// the O(depth^2) flattened-name text and the work, not the call stack).
pub const DEFAULT_MAX_SUBCKT_DEPTH: usize = 1000;
/// Default maximum number of cards visited while flattening hierarchy (instance cards + element cards).
pub const DEFAULT_MAX_EXPANDED_CARDS: usize = 2_000_000;
/// Default maximum total text of the flattened netlist in bytes.
pub const DEFAULT_MAX_EXPANDED_BYTES: usize = 256 << 20;
/// Default maximum number of stored sweep / time points (independent of the circuit size).
pub const DEFAULT_MAX_POINTS: u64 = 50_000_000;
/// Default maximum bytes of the stored analysis result (`points x bytes_per_point(dim)`).
pub const DEFAULT_MAX_RESULT_BYTES: u64 = 1 << 30;

/// Explicit, configurable work and memory budgets (AGENTS.md Rule 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum `X` instantiation depth.
    pub max_subckt_depth: usize,
    /// Maximum cards visited during hierarchy flattening.
    pub max_expanded_cards: usize,
    /// Maximum bytes of flattened card text.
    pub max_expanded_bytes: usize,
    /// Maximum stored `.dc` / `.tran` points.
    pub max_points: u64,
    /// Maximum bytes held by the stored result of one analysis.
    pub max_result_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_subckt_depth: DEFAULT_MAX_SUBCKT_DEPTH,
            max_expanded_cards: DEFAULT_MAX_EXPANDED_CARDS,
            max_expanded_bytes: DEFAULT_MAX_EXPANDED_BYTES,
            max_points: DEFAULT_MAX_POINTS,
            max_result_bytes: DEFAULT_MAX_RESULT_BYTES,
        }
    }
}

impl ResourceLimits {
    /// Heap bytes retained per stored point of an MNA system with `dim` unknowns: the axis value (8),
    /// the row `Vec` header (24) and the row allocation (`8 * dim` + 16 allocator overhead).
    #[inline]
    pub fn bytes_per_point(dim: usize) -> u64 {
        48 + 8 * dim as u64
    }

    /// The largest point count that satisfies both `max_points` and `max_result_bytes` for `dim` unknowns.
    #[inline]
    pub fn point_cap(&self, dim: usize) -> u64 {
        self.max_points
            .min(self.max_result_bytes / Self::bytes_per_point(dim))
    }

    /// Fails unless `requested` points (given as `f64` so that `1e300` and `+inf` are representable;
    /// NaN is refused as well) fit [`Self::point_cap`] for `dim` unknowns.
    pub fn check_points(
        &self,
        analysis: &'static str,
        requested: f64,
        dim: usize,
    ) -> SimResult<()> {
        let cap = self.point_cap(dim);
        if requested <= cap as f64 {
            return Ok(());
        }
        let bytes = requested * Self::bytes_per_point(dim) as f64;
        Err(SimError::ResourceLimit {
            resource: analysis,
            requested: format!("{requested:.6e} points ({bytes:.3e} bytes at dim {dim})"),
            limit: format!(
                "{cap} points (max_points = {}, max_result_bytes = {})",
                self.max_points, self.max_result_bytes
            ),
            hint:
                "enlarge the step, shorten the analysis, or raise --max-points / --max-result-mib"
                    .to_string(),
        })
    }
}
