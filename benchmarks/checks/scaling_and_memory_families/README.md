# scaling_and_memory_families

Decks: `benchmarks/cases/performance/perf_scal_*` (12 decks in 4 size families, run by `bench-suite`, timing on).
Scripts: the three checks below (not run by `bench-suite`; see `../README.md`). Linux only (`taskset`, `/proc`).

Each family varies one axis and carries `bench.size` and a `family:<name>` tag, so a timed run produces
log-log fits for ngspice SPARSE, ngspice KLU and RustSpice (the fits only form when timing is on).
Correctness of every member is checked against an independent oracle (scipy sparse LU, mpmath, closed form
modal solution, scipy Radau) in addition to tight ngspice; all 12 are `pass` today.

| family tag | decks | size axis (`bench.size`) | `bench.est_rss_mib` |
|---|---|---|---|
| `family:perf_scal_mesh3d_dc` | `perf_scal_mesh3d_dc_n8`, `_n11`, `_n15` (3-D resistor mesh, one `.op`) | unknowns 513 / 1332 / 3376 | 35 / 196 / 1530 |
| `family:perf_scal_parallel_diodes_dc` | `perf_scal_parallel_diodes_dc_n1000`, `_n10000`, `_n100000` (51-point `.dc`) | diodes | 29 / 61 / 370 |
| `family:perf_scal_tran_len_rc_ladder10` | `perf_scal_tran_len_rc_ladder10_1e4`, `_1e5`, `_1e6` | steps | 26 / 35 / 190 |
| `family:perf_scal_tran_len_diode_clamp_ladder5` | `perf_scal_tran_len_diode_clamp_ladder5_1e4`, `_1e5`, `_1e6` | steps | 26 / 29 / 130 |

Notes on the decks: `perf_scal_mesh3d_dc_n15` runs only ngspice KLU (SPARSE is superlinear on 3-D fill: 9 s
at n15); `perf_scal_parallel_diodes_dc_n100000` is 1.39 MB of text (one explicit D card per diode, because the
`m=` multiplier is ignored by RustSpice); the 1e6-step members take minutes with timing on (5-7 repetitions
of both ngspice solvers). `bench.est_rss_mib` covers the largest child (ngspice included); measured peaks
2026-10-27: mesh 22 / 150 / 1217 MiB, diodes 18 / 42 / 289, nonlinear tran 15 / 17 / 99, linear tran 15 / 21 /
145. `perf_scal_mesh3d_dc_n15` is the heaviest deck (~1.2 GiB RustSpice, 3.4 s on an idle core, 99 % of it in
the symbolic compile); run it under `systemd-run ... -p MemoryMax=6G`.

No deck carries a `known_gap`: the results are correct, so a `symbolic-lu-blowup` tag would be judged
`gap_closed` (it is not a performance slug), and `faer-nonlinear-slow` is registered for nonlinear systems.
The memory defect below is reported by `check_mem_scaling.py` instead; a performance-only slug (for example
`symbolic-lu-superlinear`, judged on end-to-end speedup and RSS ratio) is proposed.

## Scripts

Python 3 standard library only (no numpy). Options: `--rustspice BIN` (default `$RUSTSPICE`, else
`target/release/rustspice`), `--ngspice BIN` (default `$NGSPICE`, else `ngspice`), `--workdir DIR` (scratch,
default a new directory under `/dev/shm`). They pick idle cores themselves (`--cpu`, `--cpu-a/--cpu-b`,
`--cpus` override) and pin with `taskset`; a loaded host makes the timing-based gates unreliable.

| script | gates | exit codes |
|---|---|---|
| `check_mem_scaling.py [--quick] [--max-exp-2d 1.5] [--max-exp-3d 1.667] [--max-tran-ratio 2.5]` | peak RSS (`VmHWM`, start-up subtracted) of RustSpice and ngspice-KLU on 2-D meshes (30..100 per side) and 3-D meshes (8..14 per side): the log-log exponent in the unknowns must not exceed the band-LU bound (1.5 in 2-D, 5/3 in 3-D); transient output memory (width x steps) must stay <= 2.5x the raw output size; the 1e7-step extrapolation is printed for information. Peak ~0.7 GiB. | 0, **1 violation (defect evidence)**, 2 fit unverified or binary missing |
| `check_thread_determinism.py [--big] [--cpus 22 --cpus 22,23]` | rawfiles of 4 decks are byte-identical across 13 configurations (1 and 2 cores, `RAYON_NUM_THREADS` unset/1/2/4, `OMP_NUM_THREADS`, repeat) for RustSpice and ngspice; reports whether more cores are used at all. `--big` uses the 100x100 mesh (10,001 unknowns, ~650 MiB). | 0, 1 rawfile differs, 2 |
| `check_two_process_concurrency.py [--cpu-a N --cpu-b M] [--reps 5] [--max-slowdown 2.0]` | two simultaneous copies on two separate cores run within 2.0x of the solo minimum, with unchanged `VmHWM` and identical data, for RustSpice and ngspice on a 60x60 mesh, a 3-D n12 mesh and a 3e5-step ladder. | 0, 1, 2 (cannot pick two idle cores) |

## Expected exit codes today

* `check_mem_scaling.py` exits **1 while the defect exists**: RustSpice peak RSS grows with exponent 1.76 on
  2-D and 2.11 on 3-D meshes (60 KB and 269 KB per unknown at the largest sizes, 587 MiB at 10,001 and 722 MiB at
  2,745 unknowns) against 5.6 and 12.1 KB per unknown for ngspice-KLU; the answers are correct (<= 7e-15 V vs scipy). Transient output
  memory is 1.05..1.45x the raw output (no violation; extrapolates to ~9.5 GiB for 1e7 steps x 100 stages).
* `check_thread_determinism.py` exits 0; `RAYON_NUM_THREADS=N` creates N+1 OS threads but cpu/wall stays 1.00,
  so `run` does not use more than one core and there is no core-count speedup to measure.
* `check_two_process_concurrency.py` exits 0 on idle cores (1.00..1.02x). On a busy host it can exit 1
  spuriously: on 2026-10-27 (host load 14..35 on 32 cores) one run reported 3.38x for `mesh3d_n12` / RustSpice
  (exit 1; data identical, `VmHWM` unchanged) and an immediate rerun reported 1.10x (exit 0). Re-measure on
  idle cores before reading a failure as a defect.

Run on 2026-10-27 with the frozen `rustspice-p2d`: `check_mem_scaling.py` exit 1 (16 s; exponents 1.76 and
2.11), `check_thread_determinism.py` exit 0 (10 s), `check_two_process_concurrency.py` exit 1 then 0 (above).
Not verified: 8 and 16 cores, the 18^3 and larger meshes (RSS above the 4 GiB default cap), timing ratios on a
quiet host in this session (the builder measured RustSpice 3.1x / 9.6x / 23x slower than ngspice-KLU at
n8 / n11 / n15 and 2.0-2.3x faster than ngspice SPARSE on the diode family on an idle core).
