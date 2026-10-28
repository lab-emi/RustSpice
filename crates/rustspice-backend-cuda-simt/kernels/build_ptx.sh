#!/usr/bin/env bash
# Rebuilds batch_newton.ptx (embedded by rustspice-backend-cuda-simt) from batch_newton.rs with cuda-oxide.
#
#   CARGO_OXIDE=/path/to/cargo-oxide CUDA_TOOLKIT_PATH=/path/to/cuda ./build_ptx.sh [sm_XX]
#
# The PTX targets sm_80 by default; the driver JIT-compiles it for any newer GPU. FMA contraction is off
# (--no-fmad) so that the kernel rounds like the CPU reference: the only fused multiply-adds are the explicit
# `mul_add` calls, which the CPU code makes too.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
arch=${1:-sm_80}
oxide=${CARGO_OXIDE:-cargo-oxide}
cd "$here/gpu"
"$oxide" build --no-fmad --arch "$arch"
cp rustspice_batch_kernels.ptx "$here/batch_newton.ptx"
echo "wrote $here/batch_newton.ptx ($(wc -c < "$here/batch_newton.ptx") bytes, $arch)"
