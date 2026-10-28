//! Device code of the batched DC Newton solver: one CUDA thread per parameter corner, running the same source as
//! the CPU reference (`../batch_newton.rs`). The host side of RustSpice loads the PTX through the CUDA driver; this
//! binary only exists so that cuda-oxide compiles the kernel.

use cuda_device::{kernel, thread};
use cuda_host::cuda_module;

#[path = "../../batch_newton.rs"]
mod batch_newton;

#[cuda_module]
mod kernels {
    use super::*;

    /// Solves corner `c = global thread index` (`c < corners`): status and Newton iterations to `out[2c]` and
    /// `out[2c + 1]`, the solution to the `x` slots of the workspace (slot-major, stride `corners`).
    #[kernel]
    pub fn batch_newton_dc(
        pu: &[u32],
        pf: &[f64],
        params: &[f64],
        corners: u32,
        work: *mut f64,
        out: *mut u32,
    ) {
        let c = thread::index_1d().get();
        if c >= corners as usize {
            return;
        }
        let w = batch_newton::Work {
            w: work,
            stride: corners as usize,
            corner: c,
        };
        // SAFETY: the host sizes `work` as `workspace_slots(pu) * corners` words and `out` as `2 * corners`; each
        // thread touches only its own column.
        let (st, it) = unsafe { batch_newton::solve_corner(pu, pf, params, corners as usize, c, w) };
        unsafe {
            *out.add(2 * c) = st;
            *out.add(2 * c + 1) = it;
        }
    }
}

fn main() {}
