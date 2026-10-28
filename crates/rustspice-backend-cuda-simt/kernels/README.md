# Batched DC kernel

`batch_newton.rs` is the per-corner DC Newton solve of `rustspice-backend-cuda-simt::batch`: plain diodes, level-1
MOSFETs and their plain bulk junctions evaluated as the CPU backend does, the Jacobian gathered with the CPU's tables
and factored with a static-pivot LU stream. It is compiled twice from the same source:

- by the normal RustSpice build, `include!`d into `src/batch.rs` as the rayon `cpu_ref` executor;
- by [cuda-oxide](https://github.com/NVIDIA/cuda-rust) (NVIDIA's Rust-to-PTX `rustc` backend) through the small
  crate in `gpu/`, into `batch_newton.ptx`, which `src/batch.rs` embeds with `include_str!` and the CUDA driver
  JIT-compiles at run time.

The PTX is committed, so building RustSpice needs neither CUDA nor cuda-oxide; running the GPU path needs only an
NVIDIA driver (`libcuda`). Rebuild the PTX after every change to `batch_newton.rs`, and run
`RUSTSPICE_REQUIRE_GPU=1 cargo test --release -p rustspice-solver --test batch_dc` on a GPU machine: it compares the
GPU with `cpu_ref` (relative tolerance `1e-12`), `cpu_ref` with the CPU solver's batch API (`1e-9`, the Newton
tolerance) and every converged corner with a KCL check written from the circuit description (the diode equation
and the SPICE level-1 MOSFET equations with their bulk junctions).

## Rebuilding `batch_newton.ptx`

cuda-oxide is early-stage and pins its own toolchain (`gpu/rust-toolchain.toml`: Rust 1.99.1 with `rustc-dev`,
`rust-src`, `llvm-tools`). It also needs an LLVM `llc` of version 21 or newer with the NVPTX target and the CUDA
headers / `libdevice`. A full CUDA toolkit is not required: the NVIDIA PyPI wheels provide what is needed.

```sh
# CUDA 13.x pieces from PyPI (any virtualenv tool)
uv venv cuda-venv && uv pip install --python cuda-venv \
  nvidia-cuda-nvcc nvidia-cuda-runtime nvidia-cuda-crt nvidia-nvvm nvidia-nvjitlink
export CUDA_TOOLKIT_PATH=$PWD/cuda-venv/lib/python3.*/site-packages/nvidia/cu13
export CUDA_OXIDE_LLC=$(command -v llc)      # LLVM >= 21

# cargo-oxide at the revision the kernel crate pins (gpu/Cargo.toml)
git clone https://github.com/NVIDIA/cuda-rust && git -C cuda-rust checkout a4a939a9b5f01f3bf712e92386922e1bc7c434a9
(cd cuda-rust/cuda-oxide && cargo build -p cargo-oxide)

CARGO_OXIDE=$PWD/cuda-rust/cuda-oxide/target/debug/cargo-oxide ./build_ptx.sh   # sm_80 by default
```

`build_ptx.sh` passes `--no-fmad`: the only fused multiply-adds are the explicit `mul_add` calls, which the CPU code
makes too, so both builds round alike (`exp` / `ln` come from libdevice on the GPU and libm on the CPU and may differ
in the last bit). The build is reproducible: the same source gives the same PTX bytes.

## Memory

A corner needs `BatchPlan::workspace_slots()` words of `f64` (390 for the 32-diode cascade of b6, 2,016 for the
100-stage CMOS inverter chain of b4). The GPU solver
runs at most `budget / (8 * (slots + params + 1))` corners per launch, with the budget half the free device memory
and at most 2 GiB unless `CudaBatchSolver::launch_budget` says otherwise. Host memory is the solution vectors
(`8 * n` bytes per corner) plus one launch's staging copy.
