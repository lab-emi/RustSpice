//! Compressed Sparse Column (`CscMatrix`), Compressed Sparse Row (`CsrMatrix`),
//! and conflict-free pre-compiled gather tables (`CscGatherTable`) eliminating atomic
//! contention and binary searches during Jacobian assembly.

use serde::{Deserialize, Serialize};

/// Compressed Sparse Column (CSC) matrix with pre-cached diagonal indices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CscMatrix<T> {
    pub nrows: usize,
    pub ncols: usize,
    pub col_ptr: Vec<usize>,
    pub row_idx: Vec<usize>,
    pub values: Vec<T>,
    /// Pre-computed index into `values` for each diagonal entry `(j, j)`.
    pub diag_idx: Vec<usize>,
}

impl<T: Copy + Default> CscMatrix<T> {
    /// Builds a canonical sorted [`CscMatrix`] from a list of `(row, col)` coordinates,
    /// automatically ensuring all diagonal entries `(i, i)` are present in the sparsity pattern.
    pub fn from_pattern(nrows: usize, ncols: usize, coords: &[(usize, usize)]) -> Self {
        let mut col_rows: Vec<Vec<usize>> = vec![Vec::new(); ncols];
        let min_dim = nrows.min(ncols);
        for (d, row_list) in col_rows.iter_mut().enumerate().take(min_dim) {
            row_list.push(d);
        }
        for &(r, c) in coords {
            if r < nrows && c < ncols {
                col_rows[c].push(r);
            }
        }

        let mut col_ptr = Vec::with_capacity(ncols + 1);
        let mut row_idx = Vec::new();
        let mut diag_idx = vec![usize::MAX; min_dim];
        col_ptr.push(0);

        for (c, rows) in col_rows.iter_mut().enumerate() {
            rows.sort_unstable();
            rows.dedup();
            for &r in rows.iter() {
                if r == c && c < min_dim {
                    diag_idx[c] = row_idx.len();
                }
                row_idx.push(r);
            }
            col_ptr.push(row_idx.len());
        }

        let nnz = row_idx.len();
        Self {
            nrows,
            ncols,
            col_ptr,
            row_idx,
            values: vec![T::default(); nnz],
            diag_idx,
        }
    }

    /// Returns the number of structural non-zero entries (`nnz`).
    #[inline]
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Resets all numerical non-zero values to `T::default()` without reallocating.
    #[inline]
    pub fn clear_values(&mut self) {
        self.values.fill(T::default());
    }

    /// Looks up the index into `self.values` for entry `(row, col)` using binary search
    /// within column `col`. Used only during topology pre-compilation, never in the hot loop.
    #[inline]
    pub fn find_value_index(&self, row: usize, col: usize) -> Option<usize> {
        if col >= self.ncols || row >= self.nrows {
            return None;
        }
        let start = self.col_ptr[col];
        let end = self.col_ptr[col + 1];
        self.row_idx[start..end]
            .binary_search(&row)
            .ok()
            .map(|offset| start + offset)
    }
}

impl CscMatrix<f64> {
    /// Adds `gmin` to the first `num_node_vars` diagonal entries in $O(N)$ time
    /// using pre-cached `diag_idx` offsets.
    #[inline]
    pub fn add_diagonal_gmin(&mut self, num_node_vars: usize, gmin: f64) {
        if gmin == 0.0 {
            return;
        }
        let limit = num_node_vars.min(self.diag_idx.len());
        for d in 0..limit {
            let idx = self.diag_idx[d];
            if idx < self.values.len() {
                self.values[idx] += gmin;
            }
        }
    }

    /// Computes Sparse Matrix-Vector product $y \leftarrow A x$.
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        y.fill(0.0);
        for (col, &xj) in x.iter().enumerate().take(self.ncols) {
            if xj == 0.0 {
                continue;
            }
            let start = self.col_ptr[col];
            let end = self.col_ptr[col + 1];
            for idx in start..end {
                y[self.row_idx[idx]] += self.values[idx] * xj;
            }
        }
    }

    /// Computes entrywise-absolute product $y_i \leftarrow \sum_j |A_{ij}| \cdot |x_j|$
    /// for scale-aware KCL residual convergence testing.
    pub fn spmv_abs(&self, x: &[f64], y: &mut [f64]) {
        y.fill(0.0);
        for (col, &xj) in x.iter().enumerate().take(self.ncols) {
            let xj_abs = xj.abs();
            if xj_abs == 0.0 {
                continue;
            }
            let start = self.col_ptr[col];
            let end = self.col_ptr[col + 1];
            for idx in start..end {
                y[self.row_idx[idx]] += self.values[idx].abs() * xj_abs;
            }
        }
    }

    /// Computes transposed Sparse Matrix-Vector product $y \leftarrow A^T x$.
    pub fn spmv_transpose(&self, x: &[f64], y: &mut [f64]) {
        for (col, y_col) in y.iter_mut().enumerate().take(self.ncols) {
            let start = self.col_ptr[col];
            let end = self.col_ptr[col + 1];
            let mut sum = 0.0;
            for idx in start..end {
                sum += self.values[idx] * x[self.row_idx[idx]];
            }
            *y_col = sum;
        }
    }

    /// Converts this CSC matrix into a [`CsrMatrix`] for row-parallel GPU/SIMD SpMV.
    pub fn to_csr(&self) -> CsrMatrix<f64> {
        let mut row_counts = vec![0usize; self.nrows];
        for &r in &self.row_idx {
            row_counts[r] += 1;
        }
        let mut row_ptr = Vec::with_capacity(self.nrows + 1);
        row_ptr.push(0);
        for &cnt in &row_counts {
            row_ptr.push(row_ptr.last().copied().unwrap_or(0) + cnt);
        }
        let nnz = self.values.len();
        let mut col_idx = vec![0usize; nnz];
        let mut values = vec![0.0_f64; nnz];
        let mut write_cursor = row_ptr[..self.nrows].to_vec();

        for col in 0..self.ncols {
            for idx in self.col_ptr[col]..self.col_ptr[col + 1] {
                let row = self.row_idx[idx];
                let dst = write_cursor[row];
                col_idx[dst] = col;
                values[dst] = self.values[idx];
                write_cursor[row] += 1;
            }
        }

        let mut diag_idx = vec![usize::MAX; self.nrows.min(self.ncols)];
        for (r, d_slot) in diag_idx.iter_mut().enumerate() {
            for (idx, &c_val) in col_idx
                .iter()
                .enumerate()
                .take(row_ptr[r + 1])
                .skip(row_ptr[r])
            {
                if c_val == r {
                    *d_slot = idx;
                    break;
                }
            }
        }

        CsrMatrix {
            nrows: self.nrows,
            ncols: self.ncols,
            row_ptr,
            col_idx,
            values,
            diag_idx,
        }
    }
}

/// Compressed Sparse Row (CSR) matrix for high-throughput row-wise SpMV and GPU PCG solvers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsrMatrix<T> {
    pub nrows: usize,
    pub ncols: usize,
    pub row_ptr: Vec<usize>,
    pub col_idx: Vec<usize>,
    pub values: Vec<T>,
    pub diag_idx: Vec<usize>,
}

impl CsrMatrix<f64> {
    /// Computes row-wise $y \leftarrow A x$ without write contention.
    #[inline]
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        for (r, y_r) in y.iter_mut().enumerate().take(self.nrows) {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            let mut sum = 0.0_f64;
            for idx in start..end {
                sum = self.values[idx].mul_add(x[self.col_idx[idx]], sum);
            }
            *y_r = sum;
        }
    }
}

/// A single gather contribution referencing an index in the flat device output buffer
/// and a sign/scale factor (`+1.0` or `-1.0`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GatherTerm {
    pub buffer_idx: u32,
    pub sign: f64,
}

/// Pre-compiled conflict-free Gather Table mapping flat device conductance/RHS buffers
/// directly to [`CscMatrix`] non-zeros (`values`) and the global `rhs` vector.
///
/// Each NNZ owns a disjoint slice `nnz_terms[nnz_ptr[k]..nnz_ptr[k+1]]`, enabling
/// SIMD and GPU threads to assemble the entire sparse Jacobian with zero atomic operations.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CscGatherTable {
    pub nnz_ptr: Vec<usize>,
    pub nnz_terms: Vec<GatherTerm>,
    /// Compact list of `(nnz_index, term_start, term_end)` for non-empty NNZ buckets.
    pub active_nnz: Vec<(u32, u32, u32)>,
    pub rhs_ptr: Vec<usize>,
    pub rhs_terms: Vec<GatherTerm>,
    /// Compact list of `(row_index, term_start, term_end)` for non-empty RHS buckets.
    pub active_rhs: Vec<(u32, u32, u32)>,
}

impl CscGatherTable {
    /// Gathers flat device conductances (`cond_buf`) and current sources (`rhs_buf`)
    /// into `csc_values` and `rhs` with zero write conflicts.
    #[inline]
    pub fn gather_into(
        &self,
        cond_buf: &[f64],
        rhs_buf: &[f64],
        csc_values: &mut [f64],
        rhs: &mut [f64],
    ) {
        csc_values.fill(0.0);
        for &(k, start, end) in &self.active_nnz {
            let mut acc = 0.0_f64;
            for term in &self.nnz_terms[start as usize..end as usize] {
                acc = term.sign.mul_add(cond_buf[term.buffer_idx as usize], acc);
            }
            csc_values[k as usize] = acc;
        }

        rhs.fill(0.0);
        for &(i, start, end) in &self.active_rhs {
            let mut acc = 0.0_f64;
            for term in &self.rhs_terms[start as usize..end as usize] {
                acc = term.sign.mul_add(rhs_buf[term.buffer_idx as usize], acc);
            }
            rhs[i as usize] = acc;
        }
    }

    /// Fast-path gather that initializes `csc_values` from `static_base` (`memcpy`) and
    /// accumulates only dynamic/nonlinear device contributions from `active_nnz`.
    #[inline]
    pub fn gather_with_static_base(
        &self,
        static_base: &[f64],
        cond_buf: &[f64],
        rhs_buf: &[f64],
        csc_values: &mut [f64],
        rhs: &mut [f64],
    ) {
        csc_values.copy_from_slice(static_base);
        for &(k, start, end) in &self.active_nnz {
            let mut acc = csc_values[k as usize];
            for term in &self.nnz_terms[start as usize..end as usize] {
                acc = term.sign.mul_add(cond_buf[term.buffer_idx as usize], acc);
            }
            csc_values[k as usize] = acc;
        }

        rhs.fill(0.0);
        for &(i, start, end) in &self.active_rhs {
            let mut acc = 0.0_f64;
            for term in &self.rhs_terms[start as usize..end as usize] {
                acc = term.sign.mul_add(rhs_buf[term.buffer_idx as usize], acc);
            }
            rhs[i as usize] = acc;
        }
    }
}
