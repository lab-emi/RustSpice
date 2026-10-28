//! Topology-specialized Symbolic Sparse LU and `Cranelift` Native Machine Code JIT Compiler.
//!
//! Performs symbolic Gaussian elimination with static row pivoting once during circuit compilation,
//! recording the exact sequence of non-zero fill-ins and arithmetic micro-operations (`SymbolicLuOp`).
//! For small-to-medium circuits, compiles the entire LU factorization and forward/backward
//! substitution into a loop-free, zero-indirection native `x86_64` / `AArch64` machine code function
//! via `cranelift-jit`.

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use rustc_hash::FxHashMap;
use rustspice_core::{CscMatrix, SimError, SimResult};

/// Micro-operation in the pre-compiled symbolic LU factorization stream.
#[derive(Clone, Copy, Debug)]
pub enum SymbolicLuOp {
    /// `lu[diag_idx] = 1.0 / lu[diag_idx]`
    InvertPivot { diag_idx: u32 },
    /// `lu[lower_idx] *= lu[diag_idx]`
    ScaleMultiplier { lower_idx: u32, diag_idx: u32 },
    /// `lu[target_idx] -= lu[lower_idx] * lu[upper_idx]`
    Eliminate {
        target_idx: u32,
        lower_idx: u32,
        upper_idx: u32,
    },
}

/// Off-diagonal elimination entry `(row_or_col_index, lu_buffer_index)` for triangular solves.
#[derive(Clone, Copy, Debug)]
pub struct TriangularTerm {
    pub vec_idx: u32,
    pub lu_idx: u32,
}

/// Pre-compiled zero-indirection symbolic LU representation with static row pivoting.
#[derive(Clone, Debug)]
pub struct UnrolledSymbolicLu {
    pub dim: usize,
    pub lu_nnz: usize,
    /// Row permutation `row_perm[new_row] = old_row`.
    pub row_perm: Vec<usize>,
    /// Inverse row permutation `inv_row_perm[old_row] = new_row`.
    pub inv_row_perm: Vec<usize>,
    /// Maps each NNZ index `k` of the input `CscMatrix` to its slot in `lu_buf`.
    pub csc_to_lu: Vec<u32>,
    /// Slot in `lu_buf` holding the diagonal pivot `U[k, k]` (stored as `1.0 / U[k, k]` after factorization).
    pub diag_lu_idx: Vec<u32>,
    /// Flat sequence of numeric factorization operations.
    pub factor_ops: Vec<SymbolicLuOp>,
    /// Column pointers into `l_terms` for forward substitution $L y = P b$.
    pub l_col_ptr: Vec<usize>,
    pub l_terms: Vec<TriangularTerm>,
    /// Column pointers into `u_terms` for backward substitution $U x = y$.
    pub u_col_ptr: Vec<usize>,
    pub u_terms: Vec<TriangularTerm>,
}

impl UnrolledSymbolicLu {
    /// Relative pivot tolerance for threshold Markowitz pivoting (`0.1` instead of `1e-3`).
    pub const PIVOT_REL_TOL: f64 = 0.1;
    /// Diagonal preference threshold: when the structural diagonal `(k, k)` is unused and
    /// `|A[k, k]| >= DIAG_PREF_TOL * max_col_val`, prefer keeping `(k, k)` on the diagonal
    /// before searching off-diagonal rows.
    pub const DIAG_PREF_TOL: f64 = 0.5;
    /// Maximum tolerable growth ratio `max(|L|, |U|) / max(|A|)` before a static pivot
    /// factorization is declared numerically unstable and handed to a dynamic-pivoting fallback.
    pub const MAX_FACTOR_GROWTH: f64 = 1e12;

    /// Performs symbolic Gaussian elimination with numerical static pivoting on `initial_csc`
    /// to synthesize a zero-indirection [`UnrolledSymbolicLu`] execution plan.
    pub fn compile(initial_csc: &CscMatrix<f64>) -> SimResult<Self> {
        Self::compile_with_budget(initial_csc, usize::MAX)
    }

    /// Performs symbolic Gaussian elimination with an explicit upper bound `max_ops` on the
    /// number of elimination micro-operations. If fill-in causes the operation count to exceed
    /// `max_ops`, returns `Err` early without allocating the multi-hundred-megabyte `factor_ops` vector.
    pub fn compile_with_budget(initial_csc: &CscMatrix<f64>, max_ops: usize) -> SimResult<Self> {
        let n = initial_csc.nrows;
        if n != initial_csc.ncols || n == 0 {
            return Err(SimError::TopologyError(
                "Matrix must be non-empty and square for symbolic LU".to_string(),
            ));
        }

        // Build mutable sparse rows and column-to-row index lists for O(NNZ) Markowitz pivot discovery.
        // Register all structural entries (including entries whose DC seed value is 0.0, such as
        // unactivated MOSFET gm/gds or transient capacitor/inductor slots) so column structure is complete.
        let mut sim_rows: Vec<FxHashMap<usize, f64>> = vec![FxHashMap::default(); n];
        let mut col_to_rows: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (col, col_rows) in col_to_rows.iter_mut().enumerate().take(n) {
            for idx in initial_csc.col_ptr[col]..initial_csc.col_ptr[col + 1] {
                let row = initial_csc.row_idx[idx];
                let val = initial_csc.values[idx];
                sim_rows[row].insert(col, val);
                col_rows.push(row);
            }
        }

        let mut row_perm: Vec<usize> = vec![usize::MAX; n];
        let mut used = vec![false; n];

        // Pre-pair grounded voltage-source singleton branches `(p, br)`:
        // Only pair when `col_to_rows[br]` has a single *off-diagonal* structural entry `p != br`
        // and the `(br, br)` diagonal is zero (pure voltage source branch, not an inductor with R_eq).
        for br in 0..n {
            let br_diag = sim_rows[br].get(&br).copied().unwrap_or(0.0).abs();
            let off_diag_rows: Vec<usize> = col_to_rows[br]
                .iter()
                .copied()
                .filter(|&r| r != br && sim_rows[r].get(&br).copied().unwrap_or(0.0).abs() > 1e-9)
                .collect();
            let off_diag_cols: Vec<usize> = sim_rows[br]
                .iter()
                .filter(|&(&c, &v)| c != br && v.abs() > 1e-9)
                .map(|(&c, _)| c)
                .collect();
            if br_diag <= 1e-15 && off_diag_rows.len() == 1 && off_diag_cols.len() == 1 {
                let p = off_diag_rows[0];
                if p == off_diag_cols[0] && p != br && !used[p] && !used[br] {
                    row_perm[p] = br;
                    row_perm[br] = p;
                    used[br] = true;
                    used[p] = true;
                    for &r_orig in &col_to_rows[p] {
                        if r_orig != br {
                            sim_rows[r_orig].remove(&p);
                        }
                    }
                }
            }
        }

        let mut ops_lb: usize = 0;

        // Perform O(NNZ) Markowitz + Diagonal-Preferring Threshold Partial Pivoting to determine remaining `row_perm`
        for k in 0..n {
            if row_perm[k] != usize::MAX {
                continue;
            }
            let mut max_col_val = 0.0_f64;
            for &r_orig in &col_to_rows[k] {
                if !used[r_orig] {
                    let v = sim_rows[r_orig].get(&k).copied().unwrap_or(0.0).abs();
                    max_col_val = max_col_val.max(v);
                }
            }

            if max_col_val <= 1e-15 {
                return Err(SimError::SingularMatrix { index: k });
            }

            // Prefer the structural diagonal `(k, k)` when it is unused and within `DIAG_PREF_TOL`
            // of the column maximum. MNA nodal rows are diagonally dominant when conductive paths
            // or transient companion conductances `2C/h` are active; keeping `(k, k)` on the diagonal
            // avoids freezing an off-diagonal coupling pivot that becomes tiny in `.tran`.
            let diag_val = if !used[k] {
                sim_rows[k].get(&k).copied().unwrap_or(0.0).abs()
            } else {
                0.0
            };

            let best_orig_row = if !used[k] && diag_val >= Self::DIAG_PREF_TOL * max_col_val {
                k
            } else {
                let threshold = Self::PIVOT_REL_TOL * max_col_val;
                let mut chosen = usize::MAX;
                let mut best_deg = usize::MAX;
                let mut best_val = -1.0_f64;

                for &r_orig in &col_to_rows[k] {
                    if used[r_orig] {
                        continue;
                    }
                    let v = sim_rows[r_orig].get(&k).copied().unwrap_or(0.0).abs();
                    if v >= threshold {
                        let deg = sim_rows[r_orig].len();
                        let is_diag = r_orig == k;
                        let chosen_is_diag = chosen == k;
                        if deg < best_deg
                            || (deg == best_deg && is_diag && !chosen_is_diag)
                            || (deg == best_deg && is_diag == chosen_is_diag && v > best_val)
                        {
                            best_deg = deg;
                            best_val = v;
                            chosen = r_orig;
                        }
                    }
                }
                chosen
            };

            if best_orig_row == usize::MAX {
                return Err(SimError::SingularMatrix { index: k });
            }

            row_perm[k] = best_orig_row;
            used[best_orig_row] = true;

            let piv_val = sim_rows[best_orig_row].get(&k).copied().unwrap_or(1.0);
            let piv_upper: Vec<(usize, f64)> = sim_rows[best_orig_row]
                .iter()
                .filter(|&(&c, &v)| c > k && v.abs() > 0.0)
                .map(|(&c, &v)| (c, v))
                .collect();

            let candidate_rows = col_to_rows[k].clone();
            for r_orig in candidate_rows {
                if used[r_orig] {
                    continue;
                }
                if let Some(&lik) = sim_rows[r_orig].get(&k) {
                    if lik.abs() == 0.0 {
                        continue;
                    }
                    ops_lb = ops_lb.saturating_add(1 + piv_upper.len());
                    if ops_lb > max_ops {
                        return Err(SimError::TopologyError(
                            "Symbolic LU operation budget exceeded during pivot search".to_string(),
                        ));
                    }
                    let mult = lik / piv_val;
                    sim_rows[r_orig].remove(&k);
                    for &(c, ukj) in &piv_upper {
                        let is_new = !sim_rows[r_orig].contains_key(&c);
                        let entry = sim_rows[r_orig].entry(c).or_insert(0.0);
                        *entry -= mult * ukj;
                        if is_new {
                            col_to_rows[c].push(r_orig);
                        }
                    }
                }
            }
        }

        let mut inv_row_perm = vec![0usize; n];
        for (new_r, &old_r) in row_perm.iter().enumerate() {
            inv_row_perm[old_r] = new_r;
        }

        // Trace exact symbolic sparsity structure under `row_perm` using compact sorted Vecs
        let mut active_rows: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut sym_col_rows: Vec<Vec<usize>> = vec![Vec::new(); n];
        for col in 0..n {
            for idx in initial_csc.col_ptr[col]..initial_csc.col_ptr[col + 1] {
                let new_row = inv_row_perm[initial_csc.row_idx[idx]];
                active_rows[new_row].push(col);
                if new_row > col {
                    sym_col_rows[col].push(new_row);
                }
            }
            active_rows[col].push(col);
        }
        for row_vec in &mut active_rows {
            row_vec.sort_unstable();
            row_vec.dedup();
        }

        let mut row_upper_cols: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut ops_exact: usize = n;
        for k in 0..n {
            let split_idx = active_rows[k].partition_point(|&c| c <= k);
            let upper_cols: Vec<usize> = active_rows[k][split_idx..].to_vec();
            let mut lower_rows = sym_col_rows[k].clone();
            lower_rows.sort_unstable();
            lower_rows.dedup();
            let num_lower = lower_rows.iter().filter(|&&i| i > k).count();
            ops_exact = ops_exact.saturating_add(num_lower * (1 + upper_cols.len()));
            if ops_exact > max_ops {
                return Err(SimError::TopologyError(
                    "Symbolic LU operation budget exceeded during symbolic fill-in".to_string(),
                ));
            }
            if !upper_cols.is_empty() {
                for i in lower_rows {
                    if i > k {
                        for &c in &upper_cols {
                            if let Err(pos) = active_rows[i].binary_search(&c) {
                                active_rows[i].insert(pos, c);
                                if i > c {
                                    sym_col_rows[c].push(i);
                                }
                            }
                        }
                    }
                }
            }
            row_upper_cols[k] = upper_cols;
        }

        // Allocate flat `lu_buf` slots row-by-row using O(1) row offset + binary search
        let mut row_lu_start = Vec::with_capacity(n + 1);
        row_lu_start.push(0u32);
        let mut diag_lu_idx = vec![0u32; n];

        for k in 0..n {
            let start = *row_lu_start.last().unwrap();
            let diag_offset = active_rows[k]
                .binary_search(&k)
                .expect("Diagonal must be present in active_rows")
                as u32;
            diag_lu_idx[k] = start + diag_offset;
            row_lu_start.push(start + active_rows[k].len() as u32);
        }
        let lu_nnz = *row_lu_start.last().unwrap();

        let lookup_lu_idx = |r: usize, c: usize| -> u32 {
            let offset = active_rows[r]
                .binary_search(&c)
                .expect("Structural entry must exist in active_rows")
                as u32;
            row_lu_start[r] + offset
        };

        let mut csc_to_lu = Vec::with_capacity(initial_csc.nnz());
        for col in 0..n {
            for idx in initial_csc.col_ptr[col]..initial_csc.col_ptr[col + 1] {
                let new_row = inv_row_perm[initial_csc.row_idx[idx]];
                let lu_id = lookup_lu_idx(new_row, col);
                csc_to_lu.push(lu_id);
            }
        }

        // Build `factor_ops`, `l_terms`, and `u_terms`
        let mut factor_ops = Vec::new();
        let mut l_col_ptr = Vec::with_capacity(n + 1);
        let mut l_terms = Vec::new();
        l_col_ptr.push(0);

        let mut u_col_lists: Vec<Vec<TriangularTerm>> = vec![Vec::new(); n];

        for k in 0..n {
            let diag_id = diag_lu_idx[k];
            factor_ops.push(SymbolicLuOp::InvertPivot { diag_idx: diag_id });

            let upper_cols = &row_upper_cols[k];
            for &c in upper_cols {
                let u_id = lookup_lu_idx(k, c);
                u_col_lists[c].push(TriangularTerm {
                    vec_idx: k as u32,
                    lu_idx: u_id,
                });
            }

            let mut lower_rows = sym_col_rows[k].clone();
            lower_rows.sort_unstable();
            lower_rows.dedup();
            for i in lower_rows {
                if i > k {
                    if let Ok(off) = active_rows[i].binary_search(&k) {
                        let l_ik = row_lu_start[i] + off as u32;
                        factor_ops.push(SymbolicLuOp::ScaleMultiplier {
                            lower_idx: l_ik,
                            diag_idx: diag_id,
                        });
                        l_terms.push(TriangularTerm {
                            vec_idx: i as u32,
                            lu_idx: l_ik,
                        });
                        for &c in upper_cols {
                            let u_kc = lookup_lu_idx(k, c);
                            let target_ic = lookup_lu_idx(i, c);
                            factor_ops.push(SymbolicLuOp::Eliminate {
                                target_idx: target_ic,
                                lower_idx: l_ik,
                                upper_idx: u_kc,
                            });
                        }
                    }
                }
            }
            l_col_ptr.push(l_terms.len());
        }

        let mut u_col_ptr = Vec::with_capacity(n + 1);
        let mut u_terms = Vec::new();
        u_col_ptr.push(0);
        for col_list in u_col_lists.iter().take(n) {
            u_terms.extend_from_slice(col_list);
            u_col_ptr.push(u_terms.len());
        }

        Ok(Self {
            dim: n,
            lu_nnz: lu_nnz as usize,
            row_perm,
            inv_row_perm,
            csc_to_lu,
            diag_lu_idx,
            factor_ops,
            l_col_ptr,
            l_terms,
            u_col_ptr,
            u_terms,
        })
    }

    /// Scatters `csc_values` into `lu_buf` and executes the pre-compiled symbolic LU operations.
    #[inline]
    pub fn factorize(&self, csc_values: &[f64], lu_buf: &mut [f64]) -> SimResult<()> {
        lu_buf.fill(0.0);
        for (csc_idx, &lu_idx) in self.csc_to_lu.iter().enumerate() {
            lu_buf[lu_idx as usize] = csc_values[csc_idx];
        }

        for op in &self.factor_ops {
            match *op {
                SymbolicLuOp::InvertPivot { diag_idx } => {
                    let piv = lu_buf[diag_idx as usize];
                    if piv.abs() < 1e-30 || !piv.is_finite() {
                        return Err(SimError::SingularMatrix {
                            index: diag_idx as usize,
                        });
                    }
                    lu_buf[diag_idx as usize] = 1.0 / piv;
                }
                SymbolicLuOp::ScaleMultiplier {
                    lower_idx,
                    diag_idx,
                } => {
                    lu_buf[lower_idx as usize] *= lu_buf[diag_idx as usize];
                }
                SymbolicLuOp::Eliminate {
                    target_idx,
                    lower_idx,
                    upper_idx,
                } => {
                    let prod = lu_buf[lower_idx as usize] * lu_buf[upper_idx as usize];
                    lu_buf[target_idx as usize] -= prod;
                }
            }
        }
        if !self.factors_are_stable(csc_values, lu_buf) {
            return Err(SimError::SingularMatrix { index: 0 });
        }
        Ok(())
    }

    /// Verifies that the computed `lu_buf` factors did not suffer catastrophic element growth
    /// (`max(|L|, |U|) / max(|A|) <= MAX_FACTOR_GROWTH`) or near-zero pivots relative to `||A||_max`.
    #[inline]
    pub fn factors_are_stable(&self, csc_values: &[f64], lu_buf: &[f64]) -> bool {
        let mut max_a = 0.0_f64;
        for &v in csc_values {
            if !v.is_finite() {
                return false;
            }
            max_a = max_a.max(v.abs());
        }
        if max_a == 0.0 {
            return false;
        }
        // Check diagonal pivots (stored as 1.0 / U[k, k] in diag_lu_idx)
        let mut max_u_diag = 0.0_f64;
        let mut min_piv = f64::INFINITY;
        for &d_idx in &self.diag_lu_idx {
            let inv_piv = lu_buf[d_idx as usize];
            if !inv_piv.is_finite() || inv_piv == 0.0 {
                return false;
            }
            let piv_abs = (1.0 / inv_piv).abs();
            max_u_diag = max_u_diag.max(piv_abs);
            min_piv = min_piv.min(piv_abs);
        }
        // Check off-diagonal L multipliers and U entries
        let mut max_lu = max_u_diag;
        for term in &self.l_terms {
            let v = lu_buf[term.lu_idx as usize];
            if !v.is_finite() {
                return false;
            }
            max_lu = max_lu.max(v.abs());
        }
        for term in &self.u_terms {
            let v = lu_buf[term.lu_idx as usize];
            if !v.is_finite() {
                return false;
            }
            max_lu = max_lu.max(v.abs());
        }
        max_lu <= Self::MAX_FACTOR_GROWTH * max_a && min_piv >= 1e-12 * max_a
    }

    /// Solves $A x = b$ using the pre-factored `lu_buf`.
    #[inline]
    pub fn solve(&self, lu_buf: &[f64], rhs: &[f64], x: &mut [f64]) {
        let n = self.dim;
        for (new_r, &old_r) in self.row_perm.iter().enumerate() {
            x[new_r] = rhs[old_r];
        }

        // Forward substitution: L * y = P * b
        for k in 0..n {
            let xk = x[k];
            if xk != 0.0 {
                let start = self.l_col_ptr[k];
                let end = self.l_col_ptr[k + 1];
                for term in &self.l_terms[start..end] {
                    x[term.vec_idx as usize] -= lu_buf[term.lu_idx as usize] * xk;
                }
            }
        }

        // Backward substitution: U * x = y (note: diag_lu_idx stores 1.0 / U[k, k])
        for k in (0..n).rev() {
            let xk = x[k] * lu_buf[self.diag_lu_idx[k] as usize];
            x[k] = xk;
            if xk != 0.0 {
                let start = self.u_col_ptr[k];
                let end = self.u_col_ptr[k + 1];
                for term in &self.u_terms[start..end] {
                    x[term.vec_idx as usize] -= lu_buf[term.lu_idx as usize] * xk;
                }
            }
        }
    }

    /// Solves the transposed system $A^T \lambda = b$ using the pre-factored `lu_buf`
    /// for Discrete Adjoint Sensitivity Analysis:
    /// $(P^T L U)^T \lambda = U^T L^T P \lambda = b \implies U^T z = b, L^T w = z, \lambda = P^T w$.
    #[inline]
    pub fn solve_transpose(
        &self,
        lu_buf: &[f64],
        rhs: &[f64],
        lambda: &mut [f64],
        work: &mut [f64],
    ) {
        let n = self.dim;
        work[..n].copy_from_slice(&rhs[..n]);

        // Forward solve with U^T: U^T * z = rhs
        for k in 0..n {
            let start = self.u_col_ptr[k];
            let end = self.u_col_ptr[k + 1];
            let mut sum = work[k];
            for term in &self.u_terms[start..end] {
                sum -= lu_buf[term.lu_idx as usize] * work[term.vec_idx as usize];
            }
            work[k] = sum * lu_buf[self.diag_lu_idx[k] as usize];
        }

        // Backward solve with L^T: L^T * w = z
        for k in (0..n).rev() {
            let start = self.l_col_ptr[k];
            let end = self.l_col_ptr[k + 1];
            let mut sum = work[k];
            for term in &self.l_terms[start..end] {
                sum -= lu_buf[term.lu_idx as usize] * work[term.vec_idx as usize];
            }
            work[k] = sum;
        }

        // Apply inverse row permutation: lambda[old_r] = work[new_r]
        for (new_r, &old_r) in self.row_perm.iter().enumerate() {
            lambda[old_r] = work[new_r];
        }
    }
}

type JitFactorSolveFn = unsafe extern "C" fn(*const f64, *mut f64, *const f64, *mut f64) -> i32;

/// Native machine code JIT compiler powered by `cranelift-jit`.
///
/// Compiles a circuit's [`UnrolledSymbolicLu`] directly into an `x86_64` or `AArch64`
/// native function with hardcoded memory byte offsets, zero loops, and zero index array lookups.
pub struct CraneliftTopologyJit {
    pub symbolic: UnrolledSymbolicLu,
    jit_fn: Option<JitFactorSolveFn>,
    _module: Option<JITModule>,
}

// Safety: Once compiled and finalized, the Cranelift JIT function pointer is immutable executable memory.
unsafe impl Send for CraneliftTopologyJit {}
unsafe impl Sync for CraneliftTopologyJit {}

impl CraneliftTopologyJit {
    /// Maximum number of symbolic LU micro-ops to unroll into a single native JIT function
    /// (keeps generated machine code inside L1 instruction cache and compile time < 0.2 ms;
    /// larger circuits use the zero-compile-overhead `UnrolledSymbolicLu` stream).
    pub const MAX_JIT_OPS: usize = 32;

    /// Compiles a topology-specialized native machine code solver from `initial_csc`.
    pub fn compile(initial_csc: &CscMatrix<f64>) -> SimResult<Self> {
        Self::compile_with_budget(initial_csc, usize::MAX)
    }

    /// Compiles a topology-specialized solver with an explicit upper bound `max_ops` on
    /// symbolic elimination micro-operations.
    pub fn compile_with_budget(initial_csc: &CscMatrix<f64>, max_ops: usize) -> SimResult<Self> {
        let symbolic = UnrolledSymbolicLu::compile_with_budget(initial_csc, max_ops)?;
        if symbolic.factor_ops.len() > Self::MAX_JIT_OPS {
            return Ok(Self {
                symbolic,
                jit_fn: None,
                _module: None,
            });
        }

        let mut flag_builder = settings::builder();
        let _ = flag_builder.set("opt_level", "speed");
        let isa_builder = cranelift_native::builder().map_err(|e| SimError::BackendError {
            backend: "cranelift-jit".to_string(),
            message: format!("Host ISA not supported: {e}"),
        })?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| SimError::BackendError {
                backend: "cranelift-jit".to_string(),
                message: format!("Failed to build ISA: {e}"),
            })?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let mut module = JITModule::new(builder);
        let mut ctx = module.make_context();
        let ptr_ty = module.target_config().pointer_type();

        // Signature: fn(csc_vals: *const f64, lu_buf: *mut f64, rhs: *const f64, x: *mut f64) -> i32
        ctx.func.signature.params.push(AbiParam::new(ptr_ty));
        ctx.func.signature.params.push(AbiParam::new(ptr_ty));
        ctx.func.signature.params.push(AbiParam::new(ptr_ty));
        ctx.func.signature.params.push(AbiParam::new(ptr_ty));
        ctx.func.signature.returns.push(AbiParam::new(types::I32));

        let mut fn_builder_ctx = FunctionBuilderContext::new();
        {
            let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);
            let entry_block = bcx.create_block();
            bcx.append_block_params_for_function_params(entry_block);
            bcx.switch_to_block(entry_block);
            bcx.seal_block(entry_block);

            let csc_ptr = bcx.block_params(entry_block)[0];
            let lu_ptr = bcx.block_params(entry_block)[1];
            let rhs_ptr = bcx.block_params(entry_block)[2];
            let x_ptr = bcx.block_params(entry_block)[3];

            let flags = MemFlags::trusted();
            let zero_f64 = bcx.ins().f64const(0.0);
            let one_f64 = bcx.ins().f64const(1.0);

            // 1. Zero-initialize fill-in slots in `lu_buf`, then copy `csc_vals` to `lu_buf`
            for idx in 0..symbolic.lu_nnz {
                let byte_off = (idx * 8) as i32;
                bcx.ins().store(flags, zero_f64, lu_ptr, byte_off);
            }
            for (csc_idx, &lu_idx) in symbolic.csc_to_lu.iter().enumerate() {
                let src_off = (csc_idx * 8) as i32;
                let dst_off = (lu_idx as usize * 8) as i32;
                let val = bcx.ins().load(types::F64, flags, csc_ptr, src_off);
                bcx.ins().store(flags, val, lu_ptr, dst_off);
            }

            // 2. Emit completely unrolled numeric LU factorization instructions
            for op in &symbolic.factor_ops {
                match *op {
                    SymbolicLuOp::InvertPivot { diag_idx } => {
                        let off = (diag_idx as usize * 8) as i32;
                        let piv = bcx.ins().load(types::F64, flags, lu_ptr, off);
                        let inv = bcx.ins().fdiv(one_f64, piv);
                        bcx.ins().store(flags, inv, lu_ptr, off);
                    }
                    SymbolicLuOp::ScaleMultiplier {
                        lower_idx,
                        diag_idx,
                    } => {
                        let l_off = (lower_idx as usize * 8) as i32;
                        let d_off = (diag_idx as usize * 8) as i32;
                        let l_val = bcx.ins().load(types::F64, flags, lu_ptr, l_off);
                        let d_val = bcx.ins().load(types::F64, flags, lu_ptr, d_off);
                        let prod = bcx.ins().fmul(l_val, d_val);
                        bcx.ins().store(flags, prod, lu_ptr, l_off);
                    }
                    SymbolicLuOp::Eliminate {
                        target_idx,
                        lower_idx,
                        upper_idx,
                    } => {
                        let t_off = (target_idx as usize * 8) as i32;
                        let l_off = (lower_idx as usize * 8) as i32;
                        let u_off = (upper_idx as usize * 8) as i32;
                        let t_val = bcx.ins().load(types::F64, flags, lu_ptr, t_off);
                        let l_val = bcx.ins().load(types::F64, flags, lu_ptr, l_off);
                        let u_val = bcx.ins().load(types::F64, flags, lu_ptr, u_off);
                        let prod = bcx.ins().fmul(l_val, u_val);
                        let upd = bcx.ins().fsub(t_val, prod);
                        bcx.ins().store(flags, upd, lu_ptr, t_off);
                    }
                }
            }

            // 3. Emit unrolled row permutation `x[new_r] = rhs[old_r]`
            for (new_r, &old_r) in symbolic.row_perm.iter().enumerate() {
                let src_off = (old_r * 8) as i32;
                let dst_off = (new_r * 8) as i32;
                let b_val = bcx.ins().load(types::F64, flags, rhs_ptr, src_off);
                bcx.ins().store(flags, b_val, x_ptr, dst_off);
            }

            // 4. Emit unrolled Forward Substitution `L * y = P * b`
            for k in 0..symbolic.dim {
                let start = symbolic.l_col_ptr[k];
                let end = symbolic.l_col_ptr[k + 1];
                if start < end {
                    let xk_off = (k * 8) as i32;
                    let xk = bcx.ins().load(types::F64, flags, x_ptr, xk_off);
                    for term in &symbolic.l_terms[start..end] {
                        let xi_off = (term.vec_idx as usize * 8) as i32;
                        let l_off = (term.lu_idx as usize * 8) as i32;
                        let xi = bcx.ins().load(types::F64, flags, x_ptr, xi_off);
                        let lik = bcx.ins().load(types::F64, flags, lu_ptr, l_off);
                        let prod = bcx.ins().fmul(lik, xk);
                        let upd = bcx.ins().fsub(xi, prod);
                        bcx.ins().store(flags, upd, x_ptr, xi_off);
                    }
                }
            }

            // 5. Emit unrolled Backward Substitution `U * x = y`
            for k in (0..symbolic.dim).rev() {
                let xk_off = (k * 8) as i32;
                let diag_off = (symbolic.diag_lu_idx[k] as usize * 8) as i32;
                let yk = bcx.ins().load(types::F64, flags, x_ptr, xk_off);
                let inv_u = bcx.ins().load(types::F64, flags, lu_ptr, diag_off);
                let xk = bcx.ins().fmul(yk, inv_u);
                bcx.ins().store(flags, xk, x_ptr, xk_off);

                let start = symbolic.u_col_ptr[k];
                let end = symbolic.u_col_ptr[k + 1];
                for term in &symbolic.u_terms[start..end] {
                    let xi_off = (term.vec_idx as usize * 8) as i32;
                    let u_off = (term.lu_idx as usize * 8) as i32;
                    let xi = bcx.ins().load(types::F64, flags, x_ptr, xi_off);
                    let uik = bcx.ins().load(types::F64, flags, lu_ptr, u_off);
                    let prod = bcx.ins().fmul(uik, xk);
                    let upd = bcx.ins().fsub(xi, prod);
                    bcx.ins().store(flags, upd, x_ptr, xi_off);
                }
            }

            let ok_code = bcx.ins().iconst(types::I32, 0);
            bcx.ins().return_(&[ok_code]);
            bcx.finalize();
        }

        let func_id = module
            .declare_function(
                "cranelift_unrolled_lu_factor_solve",
                Linkage::Export,
                &ctx.func.signature,
            )
            .map_err(|e| SimError::BackendError {
                backend: "cranelift-jit".to_string(),
                message: format!("Failed to declare JIT function: {e}"),
            })?;

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| SimError::BackendError {
                backend: "cranelift-jit".to_string(),
                message: format!("Failed to compile JIT function: {e}"),
            })?;

        module.clear_context(&mut ctx);
        module
            .finalize_definitions()
            .map_err(|e| SimError::BackendError {
                backend: "cranelift-jit".to_string(),
                message: format!("Failed to finalize JIT code memory: {e}"),
            })?;

        let code_ptr = module.get_finalized_function(func_id);
        let jit_fn: JitFactorSolveFn = unsafe { std::mem::transmute(code_ptr) };

        Ok(Self {
            symbolic,
            jit_fn: Some(jit_fn),
            _module: Some(module),
        })
    }

    /// Returns `true` if native machine code was generated via `cranelift-jit`.
    #[inline]
    pub fn has_native_jit(&self) -> bool {
        self.jit_fn.is_some()
    }

    /// Executes native Cranelift JIT machine code (or unrolled symbolic stream for larger circuits)
    /// to factorize `csc_values` into `lu_buf` and solve $A x = b$.
    #[inline]
    pub fn factor_and_solve(
        &self,
        csc_values: &[f64],
        lu_buf: &mut [f64],
        rhs: &[f64],
        x: &mut [f64],
    ) -> SimResult<()> {
        if let Some(func) = self.jit_fn {
            unsafe {
                func(
                    csc_values.as_ptr(),
                    lu_buf.as_mut_ptr(),
                    rhs.as_ptr(),
                    x.as_mut_ptr(),
                );
            }
            if self.symbolic.factors_are_stable(csc_values, lu_buf) {
                return Ok(());
            }
        }
        self.symbolic.factorize(csc_values, lu_buf)?;
        self.symbolic.solve(lu_buf, rhs, x);
        Ok(())
    }
}
