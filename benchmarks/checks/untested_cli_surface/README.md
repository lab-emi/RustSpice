# untested_cli_surface

Decks: `benchmarks/cases/capability/cap_cli_*` (7 decks, run by `bench-suite`).
Scripts: the four checks below, a shared helper and two Rust harness sources (not run by `bench-suite`; see `../README.md`).

`bench-suite` always runs `rustspice run <deck> --raw ... --stats ...`, so it cannot express `rustspice
adjoint`, `run --backend ...`, `codegen`, or the batch API. The decks here are fixtures: each one is also an
ordinary OP / `.dc` / `.tran` comparison against tight ngspice (every deck `pass`, mutation check ok), and the
scripts use them as inputs for the CLI surface.

## Decks

| deck | role |
|---|---|
| `cap_cli_adj_diode_net` | adjoint fixture: R1 R2 R3 D1.is D2.is |
| `cap_cli_adj_mos_inv_follower` | adjoint and backend fixture: inverter on its steep transition driving a body-biased follower (the deck on which `--backend metal` gives 0.7724 V instead of 0.6368 V) |
| `cap_cli_adj_mos_tgate` | adjoint and backend fixture: transmission gate, reversed-mode NMOS, PMOS in triode |
| `cap_cli_adj_wide_ladder_sources` | adjoint fixture with 20 parameters (19 R + D1) plus V, I, L and C |
| `cap_cli_backend_diode_or_dc` | backend fixture: two-diode OR selector, 41-point `.dc` |
| `cap_cli_backend_nand_chain_op` | backend fixture: 12 level-1 MOSFETs, first stage at threshold |
| `cap_cli_backend_nand_chain_tran` | backend fixture for the transient path |

All seven: status `pass` today.

## Scripts

Python 3 with **numpy and mpmath** (independent 50-digit MNA Newton oracle with implicit-function-theorem
sensitivities in `_clicommon.py`). Common options: `--bin BIN` (default `$RUSTSPICE`, else
`target/release/rustspice`), `--ngspice BIN` (default `$NGSPICE`, else `ngspice`), `--decks DIR` (default
`benchmarks/cases/capability`), `--cpus 16-31` (pin), `--json FILE`, `--strict` (known gaps count as
failures). Exit codes: **0** no unexpected violation (known gaps are reported as `KNOWN_GAP_CONFIRMED`;
`GAP_CLOSED` is announced when one is fixed), **1** unexpected violation, **2** prerequisite missing
(`unverified`, never a pass).

| script | gates |
|---|---|
| `check_adjoint_fd.py [--api-bin BIN]` | every sensitivity `rustspice adjoint` prints against the oracle (gate 1e-8 relative) and against Richardson finite differences of tightly converged ngspice (gate 1e-7); CLI surface (unknown node rc 1, missing file rc 1, missing flag rc 2, case-insensitive node names, ground gives exact 0, output bit-identical across repeats and `RAYON_NUM_THREADS`). With `--api-bin` also the library-only sensitivities (`V1.dc`, `I1.dc`, diode N/AREA, MOS L/KP) and bad-spec errors. `--inject-error 3e-8` must make gates fail. |
| `check_backend_equiv.py [--gpu-control]` | `run --backend {cpu,metal,cuda-tile,cuda-simt,wasm}` reproduces `--backend cpu` (1e-9 abs + 1e-6 rel, same row count and axis) on op, `.dc` and `.tran` decks plus generated 1100-device decks; `cpu` bit-deterministic across repeats and `RAYON_NUM_THREADS`; honesty probes (below). `--gpu-control` runs a torch CUDA process as positive control for the `nvidia-smi` probe (needs torch). |
| `check_codegen_kernels.py [--no-gpu] [--inject-error X]` | `codegen --target rust` kernels for diode, mosfet-l1, mosfet-ekv are compiled with `rustc` and compared with 50-digit mpmath closed forms (values and every analytic derivative) and with tight ngspice / the live LEVEL=2 solution; the emitted CUDA C++ is compiled with NVRTC and run in fp64 on the GPU (needs `cuda-python` and an NVIDIA device, otherwise `UNVERIFIED`); msl/wgsl are only compared structurally (statement lists equal to the Rust one). |
| `check_batch_pvt.py --batch-bin BIN` | `Simulator::run_batch_dc_op` (no CLI entry) on 40 corners: bit-identical across thread counts, corner order and one-corner batches; every node of every corner against the oracle (1e-9 V); vs `rustspice run` on the edited deck and tight ngspice. Without `--batch-bin` it prints `unverified` and exits 2. |

### Library harnesses (API-only checks)

`cap_cli_adjoint_api.rs` and `cap_cli_batch_pvt.rs` read only the public API of `rustspice-solver`. They are
not part of the workspace; build them from a private copy of the tree so the repository stays clean:

    rsync -a --exclude target <repo>/modules/RustSpice/ /dev/shm/me/tree/
    cp cap_cli_adjoint_api.rs cap_cli_batch_pvt.rs /dev/shm/me/tree/crates/rustspice-solver/examples/
    cd /dev/shm/me/tree && CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/dev/shm/me/target \
      cargo build -p rustspice-solver --example cap_cli_adjoint_api --example cap_cli_batch_pvt
    python3 check_adjoint_fd.py --api-bin /dev/shm/me/target/debug/examples/cap_cli_adjoint_api
    python3 check_batch_pvt.py  --batch-bin /dev/shm/me/target/debug/examples/cap_cli_batch_pvt

## Known gaps the scripts reproduce (no deck, no registered slug)

The suite cannot select a backend or run codegen, so these live in the scripts only. Proposed slugs:

* `backend-df64-mos-model-mismatch`: the double-float MOSFET evaluator behind `--backend metal` (and behind
  `--backend wasm` from 1024 nonlinear devices) has no body effect and no EKV branch; silent wrong answers
  with exit 0 (inverter + follower 0.7724 V vs 0.6368 V; wasm result depends on the device count).
* `codegen-ptx-empty-kernel`: `codegen --target cuda-ptx` emits a body of only `ret;`; launched on a GPU all
  output buffers keep their NaN sentinel. The cuda-tile target is real (matches the Rust kernel to 4e-15).
* `batch-linear-dirty-gmin-shunt`: a batch corner that overrides a resistor value is solved with a spurious
  1e-12 S shunt on every node (100 Meg / 300 Meg divider: 3.749719 V instead of 3.75 V). Needs the harness.
* `adjoint-output-truncated`: the CLI computes N sensitivities but prints only the first 15 without a marker
  (with >= 15 resistors the diode IS is never shown). Registered: `adjoint-coverage` (V/I source sensitivities
  cannot be requested), `batch-all-or-nothing`.
* `cli-option-silent-fallback`: `run --backend gpu|cuda|<empty>` and `codegen --model bogus --target nonsense`
  exit 0 on a default instead of refusing.
* `backend-label-not-executor`: banners name Apple Metal / NVIDIA CUDA / wasm-WebGPU on every path although
  the process never owns a CUDA context and the output is bit-identical to `cpu` on small decks; `--stats`
  has no executor field.

## Expected exit codes today

Run on 2026-10-27 with the frozen `rustspice-p2d` (no harness build, GPU present): `check_adjoint_fd.py` exit 0
(17 checks, 2 known gaps, the `api` row `UNVERIFIED`), `check_backend_equiv.py` exit 0 (79 checks, 15 known
gaps), `check_codegen_kernels.py` exit 0 (28 checks, 6 known gaps, GPU part run on an RTX 4090 sm_89),
`check_batch_pvt.py` exit 2 (harness not built, `unverified`). Not verified here: the msl and wgsl kernels
beyond structure (no Metal or WebGPU runtime), the batch and adjoint-API harnesses. Memory: < 300 MiB
(`--gpu-control` adds a torch process of ~900 MiB).
