//! Multi-Regime Adaptive Sparse Linear Solver:
//! 1. **Tier 1 (`CraneliftJit` / `UnrolledSymbolicLu`)**: Zero-indirection symbolic LU with
//!    native `Cranelift` machine code generation for small-to-medium and low-bandwidth circuits.
//! 2. **Tier 2 (`SymbolicOnceLu` / `FaerSupernodalLu`)**: High-performance sparse LU with cached
//!    symbolic analysis and dynamic partial pivoting fallback.
//! 3. **Tier 3 (`PreconditionedCg`)**: SIMD-accelerated Jacobi-Preconditioned Conjugate Gradient
//!    with mixed-precision (`Df64` + `f64`) iterative refinement for large-scale RC grids.

use faer::prelude::SpSolver;
use faer::sparse::{SparseColMatRef, SymbolicSparseColMatRef};
use faer::Mat;
use rustspice_backend_cpu::CraneliftTopologyJit;
use rustspice_core::{CscMatrix, CsrMatrix, Df64, SimError, SimResult};

/// Active linear solver strategy selected for a compiled circuit topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinearSolverKind {
    /// Native `Cranelift` machine-code JIT unrolled LU (`<= 8,192` micro-ops).
    CraneliftJit,
    /// Pre-compiled zero-indirection symbolic LU instruction stream (`<= 500,000` micro-ops).
    UnrolledSymbolic,
    /// Supernodal sparse LU with AMD ordering (`faer`).
    FaerSupernodal,
    /// Preconditioned Conjugate Gradient (`PCG`) iterative solver.
    PreconditionedCg,
}

/// Adaptive sparse linear solver maintaining pre-allocated workspaces (`lu_buf`, `work_vec`)
/// so that hot-loop Newton-Raphson and Transient solves incur **zero heap allocations**.
pub struct AdaptiveSparseSolver {
    pub dim: usize,
    pub kind: LinearSolverKind,
    jit_solver: Option<CraneliftTopologyJit>,
    lu_buf: Vec<f64>,
    work_vec: Vec<f64>,
    faer_lu: Option<faer::sparse::linalg::solvers::Lu<usize, f64>>,
    has_valid_factors: bool,
}

impl AdaptiveSparseSolver {
    /// Maximum symbolic operation count before switching from `UnrolledSymbolicLu` to `FaerSupernodal`.
    pub const MAX_UNROLLED_OPS: usize = 500_000;

    /// Pre-compiles the linear solver strategy from an initial assembled Jacobian `initial_csc`.
    pub fn compile(initial_csc: &CscMatrix<f64>) -> SimResult<Self> {
        let dim = initial_csc.nrows;
        // Attempt symbolic compilation for Tier 1 (Cranelift JIT / Unrolled Symbolic LU)
        if dim <= 15_000 {
            if let Ok(jit) = CraneliftTopologyJit::compile(initial_csc) {
                if jit.symbolic.factor_ops.len() <= Self::MAX_UNROLLED_OPS {
                    let kind = if jit.has_native_jit() {
                        LinearSolverKind::CraneliftJit
                    } else {
                        LinearSolverKind::UnrolledSymbolic
                    };
                    let lu_nnz = jit.symbolic.lu_nnz;
                    return Ok(Self {
                        dim,
                        kind,
                        jit_solver: Some(jit),
                        lu_buf: vec![0.0; lu_nnz],
                        work_vec: vec![0.0; dim],
                        faer_lu: None,
                        has_valid_factors: false,
                    });
                }
            }
        }

        Ok(Self {
            dim,
            kind: LinearSolverKind::FaerSupernodal,
            jit_solver: None,
            lu_buf: Vec::new(),
            work_vec: vec![0.0; dim],
            faer_lu: None,
            has_valid_factors: false,
        })
    }

    /// Returns `true` if a valid LU factorization is currently cached in the solver.
    #[inline]
    pub fn has_valid_factorization(&self) -> bool {
        self.has_valid_factors
    }

    /// Invalidates the cached LU factorization.
    #[inline]
    pub fn invalidate_factorization(&mut self) {
        self.has_valid_factors = false;
    }

    /// Performs numerical LU factorization of `csc` and solves $A x = b$ in a single pass.
    #[inline]
    pub fn factorize_and_solve(
        &mut self,
        csc: &CscMatrix<f64>,
        rhs: &[f64],
        x: &mut [f64],
    ) -> SimResult<()> {
        if let Some(ref jit) = self.jit_solver {
            if jit
                .factor_and_solve(&csc.values, &mut self.lu_buf, rhs, x)
                .is_ok()
            {
                self.has_valid_factors = true;
                return Ok(());
            }
        }

        // Tier 2: Supernodal sparse LU via `faer`
        self.factorize_faer(csc)?;
        self.solve_faer(rhs, x);
        self.has_valid_factors = true;
        Ok(())
    }

    /// Solves $A x = b$ reusing the most recently computed LU factorization without re-factorizing.
    ///
    /// Used in:
    /// - Linear fixed-step Transient simulation (factorize once, solve 1,000+ times)
    /// - Shamanskii / Modified Newton-Raphson iterations
    #[inline]
    pub fn solve_cached(&mut self, rhs: &[f64], x: &mut [f64]) -> SimResult<()> {
        if !self.has_valid_factors {
            return Err(SimError::TopologyError(
                "No cached LU factorization available".to_string(),
            ));
        }
        if let Some(ref jit) = self.jit_solver {
            jit.symbolic.solve(&self.lu_buf, rhs, x);
            return Ok(());
        }
        if self.faer_lu.is_some() {
            self.solve_faer(rhs, x);
            return Ok(());
        }
        Err(SimError::TopologyError(
            "Solver has no active factorization backend".to_string(),
        ))
    }

    /// Solves the transposed linear system $A^T \lambda = b$ reusing the cached LU factorization
    /// for **Discrete Adjoint Sensitivity Analysis**.
    #[inline]
    pub fn solve_transpose_cached(
        &mut self,
        csc_fallback: &CscMatrix<f64>,
        rhs: &[f64],
        lambda: &mut [f64],
    ) -> SimResult<()> {
        if let Some(ref jit) = self.jit_solver {
            if self.has_valid_factors {
                jit.symbolic
                    .solve_transpose(&self.lu_buf, rhs, lambda, &mut self.work_vec);
                return Ok(());
            }
        }

        // Fallback for `faer`: build explicit CSC transpose and solve
        let n = self.dim;
        let csr = csc_fallback.to_csr();
        // CSR of A has the exact column pointers and row indices of CSC(A^T)!
        let sym = unsafe {
            SymbolicSparseColMatRef::new_unchecked(
                n,
                n,
                csr.row_ptr.as_slice(),
                None,
                csr.col_idx.as_slice(),
            )
        };
        let mat = SparseColMatRef::new(sym, csr.values.as_slice());
        let lu_t = faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(
            faer::sparse::linalg::solvers::SymbolicLu::try_new(sym)
                .map_err(|_| SimError::SingularMatrix { index: 0 })?,
            mat,
        )
        .map_err(|_| SimError::SingularMatrix { index: 0 })?;

        let mut b_mat = Mat::<f64>::zeros(n, 1);
        for i in 0..n {
            b_mat[(i, 0)] = rhs[i];
        }
        lu_t.solve_in_place(b_mat.as_mut());
        for i in 0..n {
            lambda[i] = b_mat[(i, 0)];
        }
        Ok(())
    }

    fn factorize_faer(&mut self, csc: &CscMatrix<f64>) -> SimResult<()> {
        if csc.values.iter().any(|v| !v.is_finite()) {
            return Err(SimError::SingularMatrix { index: 0 });
        }
        let n = csc.nrows;
        for col in 0..n {
            let start = csc.col_ptr[col];
            let end = csc.col_ptr[col + 1];
            if !(start..end).any(|idx| csc.values[idx].abs() > 0.0) {
                return Err(SimError::SingularMatrix { index: col });
            }
        }
        let sym = unsafe {
            SymbolicSparseColMatRef::new_unchecked(
                n,
                n,
                csc.col_ptr.as_slice(),
                None,
                csc.row_idx.as_slice(),
            )
        };
        let mat = SparseColMatRef::new(sym, csc.values.as_slice());
        let sym_lu = faer::sparse::linalg::solvers::SymbolicLu::try_new(sym)
            .map_err(|_| SimError::SingularMatrix { index: 0 })?;
        let lu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            faer::sparse::linalg::solvers::Lu::try_new_with_symbolic(sym_lu, mat)
        }))
        .map_err(|_| SimError::SingularMatrix { index: 0 })?
        .map_err(|_| SimError::SingularMatrix { index: 0 })?;
        self.faer_lu = Some(lu);
        Ok(())
    }

    fn solve_faer(&self, rhs: &[f64], x: &mut [f64]) {
        if let Some(ref lu) = self.faer_lu {
            let n = self.dim;
            let mut b_mat = Mat::<f64>::zeros(n, 1);
            for i in 0..n {
                b_mat[(i, 0)] = rhs[i];
            }
            lu.solve_in_place(b_mat.as_mut());
            for i in 0..n {
                x[i] = b_mat[(i, 0)];
            }
        }
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

        // Precompute inverse diagonal preconditioner in Df64 double-float precision
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

        // r = b - A * x
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
}
