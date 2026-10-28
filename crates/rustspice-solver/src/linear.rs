//! Three-Tier Adaptive Sparse Linear Solver:
//! 1. **Tier 1 (`CraneliftNativeJit` / `UnrolledSymbolicLu`)**: Zero-loop native machine code
//!    or flat zero-indirection micro-op stream for low-fill circuits, with symmetric
//!    minimum-degree ordering, diagonal-preferring Markowitz pivoting, and dynamic
//!    pivot-growth fallback.
//! 2. **Tier 2 (`FaerSparseLu` / `DensePartialPivotLu`)**: Cached symbolic-once `faer` sparse
//!    LU factorization (`Parallelism::None`, reusing workspace and symbolic elimination trees
//!    across Newton and transient steps) and dense partial-pivoting LU for small/ill-conditioned
//!    topologies.
//! 3. **Tier 3 (`PreconditionedCg`)**: Jacobi-Preconditioned Conjugate Gradient for ultra-large
//!    Symmetric Positive Definite (SPD) RC/power-grid networks.

use faer::dyn_stack::{GlobalPodBuffer, PodStack};
use faer::sparse::linalg::lu::{
    factorize_symbolic_lu, LuRef, LuSymbolicParams, NumericLu, SymbolicLu,
};
use faer::sparse::linalg::SupernodalThreshold;
use faer::sparse::{SparseColMatRef, SymbolicSparseColMatRef};
use faer::{Mat, Parallelism};
use rustspice_backend_cpu::{CraneliftTopologyJit, UnrolledSymbolicLu};
use rustspice_core::{CscMatrix, CsrMatrix, Df64, SimError, SimResult};

#[inline]
fn ensure_finite(vec: &[f64]) -> SimResult<()> {
    if vec.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(SimError::SingularMatrix { index: 0 })
    }
}

/// Selected linear solver execution engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearSolverKind {
    /// Native `x86_64` / `AArch64` machine code compiled via `cranelift-jit`.
    CraneliftJit,
    /// Pre-compiled zero-indirection symbolic LU micro-operation stream.
    UnrolledSymbolic,
    /// Dense Gaussian elimination with full row partial pivoting (`dim <= 48`).
    DensePartialPivotLu,
    /// `faer` Symbolic-Once Supernodal / Simplicial Sparse LU.
    FaerSupernodal,
    /// Jacobi-Preconditioned Conjugate Gradient (`PCG`) iterative solver.
    PreconditionedCg,
}

/// Cached symmetric permutation $P A P^T$ mapping between the caller's CSC matrix and
/// a minimum-degree reordered CSC matrix for `UnrolledSymbolicLu`.
#[derive(Clone, Debug)]
struct SymmetricOrdering {
    /// `perm[new_idx] = old_idx`.
    perm: Vec<usize>,
    /// Reordered CSC structure $P A P^T$.
    reordered_csc: CscMatrix<f64>,
    /// For each NNZ slot `k_new` in `reordered_csc.values`, `csc_value_map[k_new]` is the
    /// source index in the original `csc.values`.
    csc_value_map: Vec<usize>,
    rhs_perm: Vec<f64>,
    x_perm: Vec<f64>,
}

impl SymmetricOrdering {
    /// Maximum graph-elimination edge updates before aborting greedy min-degree ordering
    /// so high-fill 2D/3D meshes route directly to `faer` AMD in $O(\text{NNZ})$ time.
    const ORDERING_WORK_BUDGET: usize = 200_000;

    /// Computes a symmetric minimum-degree ordering $P$ for `csc`.
    /// Returns `None` if the ordering work budget is exceeded (indicating a high-fill mesh
    /// better served by `faer` sparse LU).
    fn compute(csc: &CscMatrix<f64>) -> Option<Self> {
        let n = csc.nrows;
        let perm = if n <= 16 {
            (0..n).collect()
        } else {
            Self::min_degree_order(csc)?
        };

        let mut inv_perm = vec![0usize; n];
        for (new_i, &old_i) in perm.iter().enumerate() {
            inv_perm[old_i] = new_i;
        }

        // Build reordered CSC columns: column `new_c` corresponds to `old_c = perm[new_c]`
        let nnz = csc.nnz();
        let mut col_ptr = Vec::with_capacity(n + 1);
        let mut row_idx = Vec::with_capacity(nnz);
        let mut values = Vec::with_capacity(nnz);
        let mut csc_value_map = Vec::with_capacity(nnz);
        let mut diag_idx = vec![usize::MAX; n];
        col_ptr.push(0);

        let mut col_entries: Vec<(usize, usize, f64)> = Vec::new();
        for new_c in 0..n {
            let old_c = perm[new_c];
            col_entries.clear();
            for orig_k in csc.col_ptr[old_c]..csc.col_ptr[old_c + 1] {
                let old_r = csc.row_idx[orig_k];
                let new_r = inv_perm[old_r];
                col_entries.push((new_r, orig_k, csc.values[orig_k]));
            }
            col_entries.sort_unstable_by_key(|&(r, _, _)| r);
            for &(new_r, orig_k, val) in &col_entries {
                if new_r == new_c {
                    diag_idx[new_c] = row_idx.len();
                }
                row_idx.push(new_r);
                values.push(val);
                csc_value_map.push(orig_k);
            }
            col_ptr.push(row_idx.len());
        }

        let reordered_csc = CscMatrix {
            nrows: n,
            ncols: n,
            col_ptr,
            row_idx,
            values,
            diag_idx,
        };

        Some(Self {
            perm,
            reordered_csc,
            csc_value_map,
            rhs_perm: vec![0.0; n],
            x_perm: vec![0.0; n],
        })
    }

    fn min_degree_order(csc: &CscMatrix<f64>) -> Option<Vec<usize>> {
        let n = csc.nrows;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for c in 0..n {
            for idx in csc.col_ptr[c]..csc.col_ptr[c + 1] {
                let r = csc.row_idx[idx];
                if r != c {
                    adj[r].push(c);
                    adj[c].push(r);
                }
            }
        }
        for neighbors in &mut adj {
            neighbors.sort_unstable();
            neighbors.dedup();
        }

        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); n];
        for v in (0..n).rev() {
            let d = adj[v].len().min(n.saturating_sub(1));
            buckets[d].push(v);
        }

        let mut perm = Vec::with_capacity(n);
        let mut eliminated = vec![false; n];
        let mut work: usize = 0;
        let mut min_deg: usize = 0;

        for _step in 0..n {
            let mut best_v = usize::MAX;
            while min_deg < n {
                if let Some(v) = buckets[min_deg].pop() {
                    if !eliminated[v] && adj[v].len().min(n.saturating_sub(1)) == min_deg {
                        best_v = v;
                        break;
                    }
                } else {
                    min_deg += 1;
                }
            }
            if best_v == usize::MAX {
                for (v, &elim) in eliminated.iter().enumerate() {
                    if !elim {
                        best_v = v;
                        break;
                    }
                }
            }

            eliminated[best_v] = true;
            perm.push(best_v);

            let neighbors = std::mem::take(&mut adj[best_v]);
            for &u in &neighbors {
                if let Some(pos) = adj[u].iter().position(|&x| x == best_v) {
                    adj[u].swap_remove(pos);
                }
            }

            let k = neighbors.len();
            work = work.saturating_add(k * k);
            if work > Self::ORDERING_WORK_BUDGET {
                return None;
            }

            if k <= 32 {
                for i in 0..k {
                    let u = neighbors[i];
                    for &w in &neighbors[(i + 1)..k] {
                        if !adj[u].contains(&w) {
                            adj[u].push(w);
                            adj[w].push(u);
                        }
                    }
                }
            }

            for &u in &neighbors {
                let d = adj[u].len().min(n.saturating_sub(1));
                buckets[d].push(u);
                if d < min_deg {
                    min_deg = d;
                }
            }
        }

        Some(perm)
    }

    #[inline]
    fn gather_values(&mut self, orig_values: &[f64]) {
        for (dst, &src_k) in self
            .reordered_csc
            .values
            .iter_mut()
            .zip(self.csc_value_map.iter())
        {
            *dst = orig_values[src_k];
        }
    }
}

/// Dense Gaussian elimination with full row partial pivoting for small/ill-conditioned matrices.
#[derive(Clone, Debug)]
struct DenseLu {
    dim: usize,
    lu: Vec<f64>,
    piv: Vec<usize>,
    valid: bool,
}

impl DenseLu {
    fn new(dim: usize) -> Self {
        Self {
            dim,
            lu: vec![0.0; dim * dim],
            piv: (0..dim).collect(),
            valid: false,
        }
    }

    fn factorize(&mut self, csc: &CscMatrix<f64>) -> SimResult<()> {
        let n = self.dim;
        self.lu.fill(0.0);
        for (i, p) in self.piv.iter_mut().enumerate() {
            *p = i;
        }
        let mut max_a = 0.0_f64;
        for col in 0..n {
            for idx in csc.col_ptr[col]..csc.col_ptr[col + 1] {
                let row = csc.row_idx[idx];
                let v = csc.values[idx];
                if !v.is_finite() {
                    self.valid = false;
                    return Err(SimError::SingularMatrix { index: col });
                }
                max_a = max_a.max(v.abs());
                self.lu[row * n + col] = v;
            }
        }
        if max_a == 0.0 {
            self.valid = false;
            return Err(SimError::SingularMatrix { index: 0 });
        }

        let piv_tol = (1e-15 * max_a).max(1e-30);

        for k in 0..n {
            let mut best_row = k;
            let mut best_val = self.lu[k * n + k].abs();
            for i in (k + 1)..n {
                let v = self.lu[i * n + k].abs();
                if v > best_val {
                    best_val = v;
                    best_row = i;
                }
            }
            if best_val <= piv_tol || !best_val.is_finite() {
                self.valid = false;
                return Err(SimError::SingularMatrix { index: k });
            }
            if best_row != k {
                self.piv.swap(k, best_row);
                for j in 0..n {
                    self.lu.swap(k * n + j, best_row * n + j);
                }
            }
            let piv_val = self.lu[k * n + k];
            for i in (k + 1)..n {
                let mult = self.lu[i * n + k] / piv_val;
                self.lu[i * n + k] = mult;
                for j in (k + 1)..n {
                    self.lu[i * n + j] -= mult * self.lu[k * n + j];
                }
            }
        }
        self.valid = true;
        Ok(())
    }

    #[allow(clippy::needless_range_loop)]
    fn solve(&self, rhs: &[f64], x: &mut [f64]) {
        let n = self.dim;
        for i in 0..n {
            x[i] = rhs[self.piv[i]];
        }
        for i in 0..n {
            let mut sum = x[i];
            for j in 0..i {
                sum -= self.lu[i * n + j] * x[j];
            }
            x[i] = sum;
        }
        for i in (0..n).rev() {
            let mut sum = x[i];
            for j in (i + 1)..n {
                sum -= self.lu[i * n + j] * x[j];
            }
            x[i] = sum / self.lu[i * n + i];
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn solve_transpose(&self, rhs: &[f64], lambda: &mut [f64], work: &mut [f64]) {
        let n = self.dim;
        work[..n].copy_from_slice(&rhs[..n]);
        for i in 0..n {
            let mut sum = work[i];
            for j in 0..i {
                sum -= self.lu[j * n + i] * work[j];
            }
            work[i] = sum / self.lu[i * n + i];
        }
        for i in (0..n).rev() {
            let mut sum = work[i];
            for j in (i + 1)..n {
                sum -= self.lu[j * n + i] * work[j];
            }
            work[i] = sum;
        }
        for i in 0..n {
            lambda[self.piv[i]] = work[i];
        }
    }
}

/// Cached symbolic-once `faer` sparse LU solver avoiding per-Newton-iteration symbolic
/// analysis, heap allocations, and Rayon thread-pool contention.
struct FaerLuCache {
    dim: usize,
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    symbolic: SymbolicLu<usize>,
    numeric: NumericLu<usize, f64>,
    lu_stack: GlobalPodBuffer,
    solve_stack: GlobalPodBuffer,
    rhs_mat: Mat<f64>,
    valid: bool,
    /// Identifies the symbolic factorization: numeric factors only fit the symbolic structure they came from.
    epoch: u64,
}

/// Source of [`FaerLuCache::epoch`] values.
static FAER_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Clone for FaerLuCache {
    fn clone(&self) -> Self {
        let n = self.dim;
        let symbolic = self.symbolic.clone();
        let lu_req = symbolic
            .factorize_numeric_lu_req::<f64>(Parallelism::None)
            .expect("faer numeric LU scratch req");
        let solve_req = symbolic
            .solve_in_place_req::<f64>(1, Parallelism::None)
            .expect("faer solve scratch req");
        Self {
            dim: n,
            col_ptr: self.col_ptr.clone(),
            row_idx: self.row_idx.clone(),
            symbolic,
            numeric: NumericLu::new(),
            lu_stack: GlobalPodBuffer::new(lu_req),
            solve_stack: GlobalPodBuffer::new(solve_req),
            rhs_mat: Mat::<f64>::zeros(n, 1),
            valid: false,
            epoch: self.epoch,
        }
    }
}

impl std::fmt::Debug for FaerLuCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaerLuCache")
            .field("dim", &self.dim)
            .field("valid", &self.valid)
            .finish()
    }
}

impl FaerLuCache {
    fn try_new(csc: &CscMatrix<f64>) -> SimResult<Self> {
        let n = csc.nrows;
        let col_ptr = csc.col_ptr.clone();
        let row_idx = csc.row_idx.clone();
        let sym_ref = SymbolicSparseColMatRef::new_checked(n, n, &col_ptr, None, &row_idx);

        let params = LuSymbolicParams {
            supernodal_flop_ratio_threshold: SupernodalThreshold::AUTO,
            ..Default::default()
        };
        let symbolic = factorize_symbolic_lu(sym_ref, params)
            .map_err(|_| SimError::SingularMatrix { index: 0 })?;
        let lu_req = symbolic
            .factorize_numeric_lu_req::<f64>(Parallelism::None)
            .map_err(|_| SimError::SingularMatrix { index: 0 })?;
        let solve_req = symbolic
            .solve_in_place_req::<f64>(1, Parallelism::None)
            .map_err(|_| SimError::SingularMatrix { index: 0 })?;

        Ok(Self {
            dim: n,
            col_ptr,
            row_idx,
            symbolic,
            numeric: NumericLu::new(),
            lu_stack: GlobalPodBuffer::new(lu_req),
            solve_stack: GlobalPodBuffer::new(solve_req),
            rhs_mat: Mat::<f64>::zeros(n, 1),
            valid: false,
            epoch: FAER_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    fn factorize(&mut self, csc: &CscMatrix<f64>) -> SimResult<()> {
        let n = self.dim;
        if self.col_ptr != csc.col_ptr || self.row_idx != csc.row_idx {
            *self = Self::try_new(csc)?;
        }
        let mut max_a = 0.0_f64;
        for &v in &csc.values {
            if !v.is_finite() {
                self.valid = false;
                return Err(SimError::SingularMatrix { index: 0 });
            }
            max_a = max_a.max(v.abs());
        }
        if max_a == 0.0 {
            self.valid = false;
            return Err(SimError::SingularMatrix { index: 0 });
        }

        let sym_ref =
            SymbolicSparseColMatRef::new_checked(n, n, &self.col_ptr, None, &self.row_idx);
        let mat_ref = SparseColMatRef::new(sym_ref, csc.values.as_slice());

        self.valid = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.symbolic
                .factorize_numeric_lu(
                    &mut self.numeric,
                    mat_ref,
                    Parallelism::None,
                    PodStack::new(&mut self.lu_stack),
                )
                .map(|_| ())
        }));
        match result {
            Ok(Ok(())) => {
                self.valid = true;
                Ok(())
            }
            _ => {
                self.numeric = NumericLu::new();
                self.valid = false;
                Err(SimError::SingularMatrix { index: 0 })
            }
        }
    }

    #[inline]
    fn solve(&mut self, rhs: &[f64], x: &mut [f64]) -> SimResult<()> {
        if !self.valid {
            return Err(SimError::SingularMatrix { index: 0 });
        }
        let n = self.dim;
        for (i, &b) in rhs.iter().enumerate().take(n) {
            self.rhs_mat[(i, 0)] = b;
        }
        // SAFETY: `self.numeric` was populated by `self.symbolic.factorize_numeric_lu` when `self.valid`.
        let factor = unsafe { LuRef::new_unchecked(&self.symbolic, &self.numeric) };
        factor.solve_in_place_with_conj(
            faer::Conj::No,
            self.rhs_mat.as_mut(),
            Parallelism::None,
            PodStack::new(&mut self.solve_stack),
        );
        for (i, xi) in x.iter_mut().enumerate().take(n) {
            *xi = self.rhs_mat[(i, 0)];
        }
        Ok(())
    }

    #[inline]
    fn solve_transpose(&mut self, rhs: &[f64], lambda: &mut [f64]) -> SimResult<()> {
        if !self.valid {
            return Err(SimError::SingularMatrix { index: 0 });
        }
        let n = self.dim;
        for (i, &b) in rhs.iter().enumerate().take(n) {
            self.rhs_mat[(i, 0)] = b;
        }
        // SAFETY: `self.numeric` was populated by `self.symbolic.factorize_numeric_lu` when `self.valid`.
        let factor = unsafe { LuRef::new_unchecked(&self.symbolic, &self.numeric) };
        factor.solve_transpose_in_place_with_conj(
            faer::Conj::No,
            self.rhs_mat.as_mut(),
            Parallelism::None,
            PodStack::new(&mut self.solve_stack),
        );
        for (i, li) in lambda.iter_mut().enumerate().take(n) {
            let v = self.rhs_mat[(i, 0)];
            if !v.is_finite() {
                return Err(SimError::SingularMatrix { index: i });
            }
            *li = v;
        }
        Ok(())
    }
}

/// A copy of the LU factors of an [`AdaptiveSparseSolver`] (see [`AdaptiveSparseSolver::snapshot_factors`]). It
/// holds as much memory as the live factors.
#[derive(Clone)]
pub struct FactorSnapshot(SnapshotKind);

impl FactorSnapshot {
    /// Bytes of factor values held, when known (`None` for `faer` factors, whose size the library does not report:
    /// such a snapshot is as large as the live factors).
    pub fn known_bytes(&self) -> Option<usize> {
        match &self.0 {
            SnapshotKind::Unrolled { lu, .. } => Some(lu.len() * 8),
            SnapshotKind::Dense(d) => Some(d.lu.len() * 8 + d.piv.len() * 8),
            SnapshotKind::Faer { .. } => None,
        }
    }
}

#[derive(Clone)]
enum SnapshotKind {
    Unrolled {
        lu: Vec<f64>,
        jit: std::sync::Arc<CraneliftTopologyJit>,
    },
    Dense(DenseLu),
    Faer {
        numeric: Box<NumericLu<usize, f64>>,
        epoch: u64,
    },
}

/// Adaptive Sparse Linear Solver with pre-allocated numerical factorization buffers.
#[derive(Clone)]
pub struct AdaptiveSparseSolver {
    pub kind: LinearSolverKind,
    pub dim: usize,
    pub demoted_from: Option<LinearSolverKind>,
    ordering: Option<SymmetricOrdering>,
    unrolled_jit: Option<std::sync::Arc<CraneliftTopologyJit>>,
    lu_buf: Vec<f64>,
    work_buf: Vec<f64>,
    pcg_r: Vec<f64>,
    pcg_z: Vec<f64>,
    pcg_p: Vec<f64>,
    pcg_ap: Vec<f64>,
    inv_diag: Vec<f64>,
    dense_lu: Option<DenseLu>,
    faer_cache: Option<FaerLuCache>,
    factors_valid: bool,
    factors_in_faer: bool,
    factors_dense: bool,
}

impl AdaptiveSparseSolver {
    /// Maximum dimension for the unrolled symbolic / Cranelift JIT tier.
    pub const MAX_UNROLLED_DIM: usize = 8192;
    /// Maximum number of symbolic LU micro-operations for `UnrolledSymbolicLu`.
    pub const MAX_UNROLLED_OPS: usize = 500_000;
    /// Maximum dimension for dense partial-pivoting fallback.
    pub const MAX_DENSE_DIM: usize = 48;

    /// Compiles an [`AdaptiveSparseSolver`] tailored to the sparsity structure of `initial_csc`.
    pub fn compile(initial_csc: &CscMatrix<f64>) -> SimResult<Self> {
        let dim = initial_csc.nrows;
        let dense_lu = if dim <= Self::MAX_DENSE_DIM {
            Some(DenseLu::new(dim))
        } else {
            None
        };

        if let Some((ord, jit_solver, lu_nnz, kind)) = Self::build_unrolled(initial_csc) {
            return Ok(Self {
                kind,
                dim,
                demoted_from: None,
                ordering: Some(ord),
                unrolled_jit: Some(std::sync::Arc::new(jit_solver)),
                lu_buf: vec![0.0; lu_nnz],
                work_buf: vec![0.0; dim],
                pcg_r: Vec::new(),
                pcg_z: Vec::new(),
                pcg_p: Vec::new(),
                pcg_ap: Vec::new(),
                inv_diag: Vec::new(),
                dense_lu,
                faer_cache: None,
                factors_valid: false,
                factors_in_faer: false,
                factors_dense: false,
            });
        }

        let faer_cache = FaerLuCache::try_new(initial_csc).ok();
        Ok(Self {
            kind: LinearSolverKind::FaerSupernodal,
            dim,
            demoted_from: None,
            ordering: None,
            unrolled_jit: None,
            lu_buf: Vec::new(),
            work_buf: vec![0.0; dim],
            pcg_r: vec![0.0; dim],
            pcg_z: vec![0.0; dim],
            pcg_p: vec![0.0; dim],
            pcg_ap: vec![0.0; dim],
            inv_diag: vec![1.0; dim],
            dense_lu,
            faer_cache,
            factors_valid: false,
            factors_in_faer: false,
            factors_dense: false,
        })
    }

    fn build_unrolled(
        csc: &CscMatrix<f64>,
    ) -> Option<(
        SymmetricOrdering,
        CraneliftTopologyJit,
        usize,
        LinearSolverKind,
    )> {
        let dim = csc.nrows;
        if dim > Self::MAX_UNROLLED_DIM {
            return None;
        }
        // For medium/large matrices (dim >= 256), cap unrolled micro-ops to 32 * dim
        // so 2D/3D meshes route to `FaerLuCache` (with AMD ordering) instead of generating
        // hundreds of thousands of unrolled scalar micro-ops.
        let max_ops = if dim >= 256 {
            (32 * dim).min(Self::MAX_UNROLLED_OPS)
        } else {
            Self::MAX_UNROLLED_OPS
        };
        let ord = SymmetricOrdering::compute(csc)?;
        let jit = CraneliftTopologyJit::compile_with_budget(&ord.reordered_csc, max_ops).ok()?;
        let lu_nnz = jit.symbolic.lu_nnz;
        let kind = if jit.has_native_jit() {
            LinearSolverKind::CraneliftJit
        } else {
            LinearSolverKind::UnrolledSymbolic
        };
        Some((ord, jit, lu_nnz, kind))
    }

    /// Re-compiles the unrolled symbolic LU pivot schedule on `transient_csc` when transitioning
    /// from the DC operating point (`cap_geq = 0`, `ind_req = 0`) to transient time-stepping
    /// (`cap_geq = 2C/h`, `ind_req = 2L/h`), ensuring diagonally-dominant transient pivots.
    pub fn refresh_for_transient(&mut self, transient_csc: &CscMatrix<f64>) {
        self.invalidate_factorization();
        if self.dim <= Self::MAX_UNROLLED_DIM {
            if let Some((ord, jit_solver, lu_nnz, kind)) = Self::build_unrolled(transient_csc) {
                self.kind = kind;
                self.ordering = Some(ord);
                self.unrolled_jit = Some(std::sync::Arc::new(jit_solver));
                self.lu_buf.resize(lu_nnz, 0.0);
            }
        }
    }

    /// The static-pivot plan of the unrolled tier: the fill-reducing order (`perm[new] = old`), the source of each
    /// reordered non-zero in the caller's CSC values, and the symbolic LU of the reordered matrix. `None` when the
    /// solver is on another tier (faer, dense, PCG).
    pub fn unrolled_plan(&self) -> Option<(&[usize], &[usize], &UnrolledSymbolicLu)> {
        match (&self.ordering, &self.unrolled_jit) {
            (Some(ord), Some(jit)) => Some((&ord.perm, &ord.csc_value_map, &jit.symbolic)),
            _ => None,
        }
    }

    /// Invalidates cached LU factors (called when switching from DC OP to Transient or changing timestep $h$).
    #[inline]
    pub fn invalidate_factorization(&mut self) {
        self.factors_valid = false;
        self.factors_in_faer = false;
        self.factors_dense = false;
    }

    /// Returns `true` if a valid LU factorization is currently cached.
    #[inline]
    pub fn has_valid_factorization(&self) -> bool {
        self.factors_valid
    }

    /// Copies the current LU factors (`None` without valid factors).
    pub fn snapshot_factors(&self) -> Option<FactorSnapshot> {
        if !self.factors_valid {
            return None;
        }
        if self.factors_dense {
            return self
                .dense_lu
                .clone()
                .map(|d| FactorSnapshot(SnapshotKind::Dense(d)));
        }
        if self.factors_in_faer {
            let c = self.faer_cache.as_ref()?;
            return Some(FactorSnapshot(SnapshotKind::Faer {
                numeric: Box::new(c.numeric.clone()),
                epoch: c.epoch,
            }));
        }
        let jit = self.unrolled_jit.as_ref()?;
        Some(FactorSnapshot(SnapshotKind::Unrolled {
            lu: self.lu_buf.clone(),
            jit: jit.clone(),
        }))
    }

    /// Makes `snap` the current factors. Returns `false` (and changes nothing) when the solver's pivot schedule or
    /// symbolic structure is no longer the one the snapshot was taken with.
    pub fn restore_factors(&mut self, snap: &FactorSnapshot) -> bool {
        match &snap.0 {
            SnapshotKind::Unrolled { lu, jit } => {
                let same = self
                    .unrolled_jit
                    .as_ref()
                    .is_some_and(|j| std::sync::Arc::ptr_eq(j, jit))
                    && self.ordering.is_some()
                    && self.lu_buf.len() == lu.len();
                if !same {
                    return false;
                }
                self.lu_buf.copy_from_slice(lu);
                self.factors_in_faer = false;
                self.factors_dense = false;
            }
            SnapshotKind::Dense(d) => {
                if self.kind != LinearSolverKind::DensePartialPivotLu
                    || self.dense_lu.as_ref().is_none_or(|x| x.dim != d.dim)
                {
                    return false;
                }
                self.dense_lu = Some(d.clone());
                self.factors_in_faer = false;
                self.factors_dense = true;
            }
            SnapshotKind::Faer { numeric, epoch } => {
                let Some(c) = self.faer_cache.as_mut() else {
                    return false;
                };
                if c.epoch != *epoch || self.kind != LinearSolverKind::FaerSupernodal {
                    return false;
                }
                c.numeric = (**numeric).clone();
                c.valid = true;
                self.factors_in_faer = true;
                self.factors_dense = false;
            }
        }
        self.factors_valid = true;
        true
    }

    /// Factorizes `csc` and solves $A x = b$ into `x`.
    pub fn factorize_and_solve(
        &mut self,
        csc: &CscMatrix<f64>,
        rhs: &[f64],
        x: &mut [f64],
    ) -> SimResult<()> {
        if let (Some(ord), Some(jit)) = (&mut self.ordering, &self.unrolled_jit) {
            ord.gather_values(&csc.values);
            for (new_i, &old_i) in ord.perm.iter().enumerate() {
                ord.rhs_perm[new_i] = rhs[old_i];
            }
            if jit
                .factor_and_solve(
                    &ord.reordered_csc.values,
                    &mut self.lu_buf,
                    &ord.rhs_perm,
                    &mut ord.x_perm,
                )
                .is_ok()
            {
                for (new_i, &old_i) in ord.perm.iter().enumerate() {
                    x[old_i] = ord.x_perm[new_i];
                }
                self.factors_valid = true;
                self.factors_in_faer = false;
                self.factors_dense = false;
                return Ok(());
            }

            // Pivot-order mismatch (e.g. nonlinear regime switch or transient companion activation):
            // attempt to recompile `UnrolledSymbolicLu` once using the live numerical Jacobian.
            if let Ok(new_jit) = CraneliftTopologyJit::compile_with_budget(
                &ord.reordered_csc,
                Self::MAX_UNROLLED_OPS,
            ) {
                self.lu_buf.resize(new_jit.symbolic.lu_nnz, 0.0);
                if new_jit
                    .factor_and_solve(
                        &ord.reordered_csc.values,
                        &mut self.lu_buf,
                        &ord.rhs_perm,
                        &mut ord.x_perm,
                    )
                    .is_ok()
                {
                    for (new_i, &old_i) in ord.perm.iter().enumerate() {
                        x[old_i] = ord.x_perm[new_i];
                    }
                    self.unrolled_jit = Some(std::sync::Arc::new(new_jit));
                    self.factors_valid = true;
                    self.factors_in_faer = false;
                    self.factors_dense = false;
                    return Ok(());
                }
            }
        }

        // Fallback 1: Dense Gaussian elimination with full row partial pivoting for small circuits
        if let Some(dense) = &mut self.dense_lu {
            if dense.factorize(csc).is_ok() {
                dense.solve(rhs, x);
                if matches!(
                    self.kind,
                    LinearSolverKind::CraneliftJit | LinearSolverKind::UnrolledSymbolic
                ) {
                    self.demoted_from.get_or_insert(self.kind);
                }
                self.ordering = None;
                self.unrolled_jit = None;
                self.kind = LinearSolverKind::DensePartialPivotLu;
                self.factors_valid = true;
                self.factors_in_faer = false;
                self.factors_dense = true;
                return Ok(());
            }
        }

        // Fallback 2: `faer` Symbolic-Once Supernodal / Simplicial Sparse LU
        if matches!(
            self.kind,
            LinearSolverKind::CraneliftJit | LinearSolverKind::UnrolledSymbolic
        ) {
            self.demoted_from.get_or_insert(self.kind);
        }
        self.ordering = None;
        self.unrolled_jit = None;
        self.factorize_and_solve_faer(csc, rhs, x)
    }

    /// Solves $A x = b$ reusing the most recently computed LU factorization (Constant-$h$ Transient
    /// & Shamanskii Newton steps).
    #[inline]
    pub fn solve_cached(&mut self, rhs: &[f64], x: &mut [f64]) -> SimResult<()> {
        if !self.factors_valid {
            return Err(SimError::SingularMatrix { index: 0 });
        }
        if self.factors_dense {
            if let Some(dense) = &self.dense_lu {
                dense.solve(rhs, x);
                ensure_finite(x)?;
                return Ok(());
            }
        }
        if !self.factors_in_faer {
            if let (Some(ord), Some(jit)) = (&mut self.ordering, &self.unrolled_jit) {
                for (new_i, &old_i) in ord.perm.iter().enumerate() {
                    ord.rhs_perm[new_i] = rhs[old_i];
                }
                jit.symbolic
                    .solve(&self.lu_buf, &ord.rhs_perm, &mut ord.x_perm);
                for (new_i, &old_i) in ord.perm.iter().enumerate() {
                    x[old_i] = ord.x_perm[new_i];
                }
                ensure_finite(x)?;
                return Ok(());
            }
        }
        if let Some(cache) = &mut self.faer_cache {
            return cache.solve(rhs, x);
        }
        Err(SimError::SingularMatrix { index: 0 })
    }

    /// Solves the transposed system $A^T \lambda = b$ reusing the cached LU factorization
    /// for Discrete Adjoint Sensitivity Analysis.
    pub fn solve_transpose_cached(
        &mut self,
        csc: &CscMatrix<f64>,
        rhs: &[f64],
        lambda: &mut [f64],
    ) -> SimResult<()> {
        if self.factors_valid && self.factors_dense {
            if let Some(dense) = &self.dense_lu {
                dense.solve_transpose(rhs, lambda, &mut self.work_buf);
                ensure_finite(lambda)?;
                return Ok(());
            }
        }
        if self.factors_valid && !self.factors_in_faer {
            if let (Some(ord), Some(jit)) = (&mut self.ordering, &self.unrolled_jit) {
                for (new_i, &old_i) in ord.perm.iter().enumerate() {
                    ord.rhs_perm[new_i] = rhs[old_i];
                }
                jit.symbolic.solve_transpose(
                    &self.lu_buf,
                    &ord.rhs_perm,
                    &mut ord.x_perm,
                    &mut self.work_buf,
                );
                for (new_i, &old_i) in ord.perm.iter().enumerate() {
                    lambda[old_i] = ord.x_perm[new_i];
                }
                ensure_finite(lambda)?;
                return Ok(());
            }
        }

        if let Some(dense) = &mut self.dense_lu {
            if dense.factorize(csc).is_ok() {
                dense.solve_transpose(rhs, lambda, &mut self.work_buf);
                ensure_finite(lambda)?;
                return Ok(());
            }
        }

        if self.faer_cache.is_none() {
            self.faer_cache = Some(FaerLuCache::try_new(csc)?);
        }
        let cache = self.faer_cache.as_mut().unwrap();
        if !self.factors_valid || !self.factors_in_faer {
            cache.factorize(csc)?;
            self.factors_valid = true;
            self.factors_in_faer = true;
            self.factors_dense = false;
        }
        cache.solve_transpose(rhs, lambda)
    }

    fn factorize_and_solve_faer(
        &mut self,
        csc: &CscMatrix<f64>,
        rhs: &[f64],
        x: &mut [f64],
    ) -> SimResult<()> {
        if self.faer_cache.is_none() {
            self.faer_cache = Some(FaerLuCache::try_new(csc)?);
        }
        let cache = self.faer_cache.as_mut().unwrap();
        cache.factorize(csc)?;
        cache.solve(rhs, x)?;
        self.kind = LinearSolverKind::FaerSupernodal;
        self.factors_valid = true;
        self.factors_in_faer = true;
        self.factors_dense = false;
        Ok(())
    }

    /// Solves an SPD conductance system $A x = b$ using Jacobi-Preconditioned Conjugate Gradient
    /// (PCG) with `Df64` mixed-precision preconditioner support.
    pub fn solve_pcg_mixed_precision(
        csr: &CsrMatrix<f64>,
        rhs: &[f64],
        x: &mut [f64],
        tol: f64,
        max_iter: usize,
    ) -> SimResult<usize> {
        let n = csr.nrows;
        let mut r = vec![0.0_f64; n];
        let mut z = vec![0.0_f64; n];
        let mut p = vec![0.0_f64; n];
        let mut ap = vec![0.0_f64; n];

        let mut inv_diag_df64 = vec![Df64::ONE; n];
        for (i, inv_d) in inv_diag_df64.iter_mut().enumerate().take(n) {
            let d_idx = csr.diag_idx[i];
            let d_val = if d_idx < csr.values.len() && csr.values[d_idx].abs() > 1e-15 {
                csr.values[d_idx]
            } else {
                1.0
            };
            *inv_d = Df64::from_f64(1.0 / d_val);
        }

        csr.spmv(x, &mut ap);
        let mut rhs_norm = 0.0_f64;
        for i in 0..n {
            r[i] = rhs[i] - ap[i];
            rhs_norm = rhs_norm.max(rhs[i].abs());
            z[i] = (inv_diag_df64[i] * Df64::from_f64(r[i])).to_f64();
            p[i] = z[i];
        }
        let abs_tol = (tol * rhs_norm.max(1.0)).max(1e-13);

        let mut rz_old: f64 = r.iter().zip(z.iter()).map(|(ri, zi)| ri * zi).sum();
        if rz_old.abs() < 1e-30 {
            return Ok(0);
        }

        for iter in 1..=max_iter {
            csr.spmv(&p, &mut ap);
            let p_ap: f64 = p.iter().zip(ap.iter()).map(|(pi, api)| pi * api).sum();
            if p_ap.abs() < 1e-30 {
                break;
            }
            let alpha = rz_old / p_ap;
            let mut max_r = 0.0_f64;
            for i in 0..n {
                x[i] = alpha.mul_add(p[i], x[i]);
                r[i] = (-alpha).mul_add(ap[i], r[i]);
                max_r = max_r.max(r[i].abs());
            }
            if max_r <= abs_tol {
                return Ok(iter);
            }
            let mut rz_new = 0.0_f64;
            for i in 0..n {
                z[i] = (inv_diag_df64[i] * Df64::from_f64(r[i])).to_f64();
                rz_new = r[i].mul_add(z[i], rz_new);
            }
            let beta = rz_new / rz_old;
            for i in 0..n {
                p[i] = beta.mul_add(p[i], z[i]);
            }
            rz_old = rz_new;
        }

        Ok(max_iter)
    }

    /// Jacobi-Preconditioned Conjugate Gradient (`PCG`) solver for large SPD linear systems.
    pub fn solve_pcg(
        &mut self,
        csc: &CscMatrix<f64>,
        rhs: &[f64],
        x: &mut [f64],
        tol: f64,
        max_iters: usize,
    ) -> SimResult<usize> {
        let n = self.dim;
        if self.pcg_r.len() != n {
            self.pcg_r.resize(n, 0.0);
            self.pcg_z.resize(n, 0.0);
            self.pcg_p.resize(n, 0.0);
            self.pcg_ap.resize(n, 0.0);
            self.inv_diag.resize(n, 1.0);
        }

        for i in 0..n {
            let d = csc.values[csc.diag_idx[i]];
            self.inv_diag[i] = if d.abs() > 1e-18 { 1.0 / d } else { 1.0 };
        }

        // r = rhs - A * x
        csc.spmv(x, &mut self.pcg_ap);
        let mut rhs_norm_sq = 0.0_f64;
        for (i, &r_val) in rhs.iter().enumerate().take(n) {
            self.pcg_r[i] = r_val - self.pcg_ap[i];
            self.pcg_z[i] = self.inv_diag[i] * self.pcg_r[i];
            self.pcg_p[i] = self.pcg_z[i];
            rhs_norm_sq += r_val * r_val;
        }

        if rhs_norm_sq == 0.0 {
            x.fill(0.0);
            return Ok(0);
        }

        let mut rz_old: f64 = self
            .pcg_r
            .iter()
            .zip(self.pcg_z.iter())
            .map(|(r, z)| r * z)
            .sum();
        let abs_tol_sq = (tol * tol * rhs_norm_sq).max(1e-30);

        for iter in 1..=max_iters {
            csc.spmv(&self.pcg_p, &mut self.pcg_ap);
            let p_ap: f64 = self
                .pcg_p
                .iter()
                .zip(self.pcg_ap.iter())
                .map(|(p, ap)| p * ap)
                .sum();
            if p_ap.abs() < 1e-30 {
                break;
            }
            let alpha = rz_old / p_ap;
            let mut r_norm_sq = 0.0_f64;
            for (i, xi) in x.iter_mut().enumerate().take(n) {
                *xi += alpha * self.pcg_p[i];
                self.pcg_r[i] -= alpha * self.pcg_ap[i];
                r_norm_sq += self.pcg_r[i] * self.pcg_r[i];
            }
            if r_norm_sq <= abs_tol_sq {
                return Ok(iter);
            }
            let mut rz_new = 0.0_f64;
            for i in 0..n {
                self.pcg_z[i] = self.inv_diag[i] * self.pcg_r[i];
                rz_new += self.pcg_r[i] * self.pcg_z[i];
            }
            let beta = rz_new / rz_old;
            for i in 0..n {
                self.pcg_p[i] = self.pcg_z[i] + beta * self.pcg_p[i];
            }
            rz_old = rz_new;
        }

        Ok(max_iters)
    }
}
