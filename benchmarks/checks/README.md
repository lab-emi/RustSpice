# benchmarks/checks

Python (and two Rust harness) checks that belong to the new deck families but that the differential engine
cannot express: order of convergence, conservation laws, reference-free relations, determinism across thread
counts, memory scaling, card-order invariance, CLI surface (adjoint, backends, codegen, batch API).

**`bench-suite` does not run anything in this directory.** It only reads `*.sp` decks below `--cases`
(default `benchmarks/cases`), and this directory contains no deck. Run the scripts by hand (or from CI)
when you touch the solver area they cover. A script that exits non-zero is evidence, not noise: see the
"expected exit codes" section of each area README.

| area (README inside) | decks it belongs to | scripts |
|---|---|---|
| [`numbering_order_invariance`](numbering_order_invariance/README.md) | `cases/robustness/rob_ord_*` (12) | `check_card_order_sweep.py`, `check_twin_blocks.py`, `check_det_threads.py` |
| [`reference_free_relations`](reference_free_relations/README.md) | `cases/accuracy/acc_rel_*` (7) | `check_dc_sweep_relations.py`, `check_kcl_residual_diode_mesh.py`, `check_time_scale_invariance.py`, `check_tran_convergence_order.py`, `check_tran_rest_equals_op.py`; generators `gen_dc_decks.py`, `gen_tran_decks.py`; `calibrate_*.py` |
| [`tran_order_and_conservation_oracles`](tran_order_and_conservation_oracles/README.md) | `cases/accuracy/acc_ord_*` (4 series decks, 22 cases) | `check_order_convergence.py`, `check_conservation.py`, `check_existing_series_reference.py` |
| [`element_analysis_matrix_holes`](element_analysis_matrix_holes/README.md) | `cases/capability/cap_xm_*` (12) | `check_independent_oracles.py`, `check_dc_sweep_vs_op.py`, `check_hierarchy_flat_equivalence.py`, `check_instname_trigger.py` |
| [`untested_cli_surface`](untested_cli_surface/README.md) | `cases/capability/cap_cli_*` (7 fixture decks) | `check_adjoint_fd.py`, `check_backend_equiv.py`, `check_codegen_kernels.py`, `check_batch_pvt.py`; harnesses `cap_cli_adjoint_api.rs`, `cap_cli_batch_pvt.rs` |
| [`scaling_and_memory_families`](scaling_and_memory_families/README.md) | `cases/performance/perf_scal_*` (12, 4 families) | `check_mem_scaling.py`, `check_thread_determinism.py`, `check_two_process_concurrency.py` |

Deck notes (`* bench.note:`) name the script that backs a claim as `checks/<area>/<script>.py`.

## Conventions shared by all scripts

* **Binaries.** RustSpice is taken from `$RUSTSPICE` (a `--rustspice`/`--bin` option overrides it); the default
  is `target/release/rustspice` of this repository (`cargo build --release -p rustspice-cli`). ngspice is a
  black-box executable taken from `$NGSPICE` (default `ngspice` on PATH). No script contains a machine-specific
  path. A missing binary gives `unverified` (exit 2 where the script has a distinct code), never a pass.
* **Dependencies.** Python 3 with numpy; scipy for `tran_order_and_conservation_oracles`,
  `numbering_order_invariance` (matrix-exponential oracle) and `check_independent_oracles.py`; mpmath for
  `untested_cli_surface` and `check_independent_oracles.py`; `rustc` for `check_codegen_kernels.py`;
  `cuda-python` plus an NVIDIA GPU for its optional GPU part (`UNVERIFIED` without). `scaling_and_memory_families`
  uses the standard library only. Linux tools: `taskset`, `/proc`.
* **Exit codes.** 0 = all gates hold (documented known gaps are reported and do not fail unless `--strict`),
  1 = violation, 2 = tool or prerequisite missing; a few scripts add 3 (a known gap closed) and 4 (the
  `--selftest` of a gate failed). The area READMEs list them per script.
* **Scratch.** Every script writes only below `/dev/shm` (or the system temp dir): `--workdir DIR`,
  `RFR_WORK`, `TRANOC_WORK`. Nothing is written into the repository.
* **CPU pinning is optional.** Default: none. `RFR_CPUS`, `TRANOC_CPUS`, `EAMH_CPUS`, `--prefix "taskset ..."`
  and `--cpus` pin the children. The timing-based checks (`check_two_process_concurrency.py`, the wall-clock
  part of `check_thread_determinism.py`) need idle cores.
* **Resources (AGENTS.md Rule 9).** Most scripts finish in seconds with a small footprint (a few MiB to ~110 MiB
  RSS per simulator child). The largest: `check_mem_scaling.py` (0.7 GiB peak child, 16 s measured),
  `check_two_process_concurrency.py` (two copies of a 283 MiB child, 75-102 s measured),
  `check_independent_oracles.py` (34 s measured) and `check_det_threads.py --big` (~0.7 GiB, builder-measured,
  not run here). Wrap every run:

      RUSTSPICE=target/release/rustspice systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=1G \
        timeout 300 python3 benchmarks/checks/<area>/<script>.py

## Observed exit codes (2026-10-28, post-fix `target/release/rustspice`, ngspice-47)

| script | exit | meaning |
|---|---|---|
| `numbering_order_invariance/check_card_order_sweep.py` | 0 | 0 violations across all ladder and 16x16 diode-mesh card-order permutations (30 violations closed by `RC-lu-pivot`) |
| `numbering_order_invariance/check_twin_blocks.py` | 0 | 0 of 9 twin decks differ (6 violations closed by `RC-lu-pivot`) |
| `numbering_order_invariance/check_det_threads.py` | 0 | rawfiles byte-identical in all configurations |
| `reference_free_relations/check_*.py` (5) | 0 | all gates hold, also with `--selftest` |
| `tran_order_and_conservation_oracles/check_*.py` (3) | 0 | order 2.00, energy conserved to round-off |
| `element_analysis_matrix_holes/check_*.py` (4) | 0 | oracle audit, sweep path independence, hierarchy equals flat, name trigger proven |
| `untested_cli_surface/check_adjoint_fd.py` | 0 | 17 checks, 0 unexpected failures, `adjoint-output-truncated` closed (`GAP_CLOSED`) |
| `untested_cli_surface/check_backend_equiv.py` | 0 | 79 checks, 0 unexpected failures; `backend-df64-mos-model-mismatch`, `cli-backend-option-silent-cpu` and `cli-codegen-option-silent` closed (`GAP_CLOSED`) |
| `untested_cli_surface/check_codegen_kernels.py` | 0 | 28 checks, 0 unexpected failures, GPU part run (`sm_89`); `codegen-ptx-empty-kernel` and `cli-codegen-option-silent` closed (`GAP_CLOSED`) |
| `untested_cli_surface/check_batch_pvt.py` | 2 | library harness not built by default (`unverified`); `run_batch_dc_op` tested by `rustspice-solver` unit/integration tests |
| `scaling_and_memory_families/check_mem_scaling.py` | 0 | peak-RSS scaling within band-LU / sparse-LU bounds (2-D 100x100 mesh 37.5 MiB / 3.8 KB per unknown; 3-D 14x14x14 mesh 24.9 MiB / 9.3 KB per unknown, smaller than ngspice-KLU; closed by `RC-faer-slow`) |
| `scaling_and_memory_families/check_thread_determinism.py` | 0 | 13 configurations identical |
| `scaling_and_memory_families/check_two_process_concurrency.py` | 0 | contention-sensitive on shared hosts; passes on idle cores |

Historical pre-fix findings (2026-10-27, frozen `rustspice-p2d`) and root-cause notes are preserved in the area READMEs and `../proposed-fixes/README.md`.
