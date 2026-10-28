# numbering_order_invariance

Decks: `benchmarks/cases/robustness/rob_ord_*` (12 decks, run by `bench-suite`).
Scripts: the three below (not run by `bench-suite`; see `../README.md` for the conventions).

Premise: shuffling the cards of a deck (node names unchanged) does not change the physics. ngspice is
card-order invariant (<= 7.5e-14 of swing over 90 shuffled runs); RustSpice is not on linear transients from
22 unknowns upward (finite wrong values, 1e45 V, inf/NaN, all with exit 0), and on some nonlinear meshes and
ladders (loud `ConvergenceFailure`, runtime blow-up). Same frozen-pivot LU mechanism as the registered slug
`unrolled-lu-tran-divergence`; the root cause is not confirmed by an instrumented build.

## Decks

| deck | what | expected status today |
|---|---|---|
| `rob_ord_rcladder10_cardshuffle_s1` | 10-stage RC ladder, shuffled; size-axis control | pass |
| `rob_ord_rcladder40_natural` | 40-stage RC ladder, natural order; control for the shuffled copies | pass |
| `rob_ord_cmos_chain8_tran_cardshuffle` | 8-stage CMOS chain, shuffled; nonlinear control | pass |
| `rob_ord_diode_mesh8_dc_cardshuffle` | 8x8 diode mesh `.dc`, shuffled; nonlinear control | pass |
| `rob_ord_rcladder20_cardshuffle_s1` | 20 stages, 22 unknowns, exponential garbage (CraneliftJit tier) | known_gap_confirmed |
| `rob_ord_rcladder40_cardshuffle_s4` | 40 stages, finite but wrong waveforms | known_gap_confirmed |
| `rob_ord_rcladder60_cardshuffle_s2` | 60 stages, inf/NaN in the rawfile, exit 0 | known_gap_confirmed |
| `rob_ord_lcladder10_cardshuffle_s1` | LC ladder, values up to 1e45 V | known_gap_confirmed |
| `rob_ord_twin_rcladder30_blocks` | two identical ladders in one netlist, block B shuffled | known_gap_confirmed |
| `rob_ord_perm_perf_lin_rc_ladder30_pulse` | cards of `perf_lin_rc_ladder30_pulse_1e5` shuffled | known_gap_confirmed |
| `rob_ord_diode_mesh16_dc_cardshuffle_s3` | 16x16 diode mesh, loud `ConvergenceFailure` | known_gap_confirmed (`linear-static-pivot-accuracy`, closest registered slug) |
| `rob_ord_diode_rc_ladder100_cardshuffle_hang` | diode-clamped 100-stage line, 267 s CPU instead of 0.2 s | known_gap_confirmed by the 60 s case timeout |

`rob_ord_diode_rc_ladder100_cardshuffle_hang` costs one full `--case-timeout` per run and is host-speed
dependent: a much faster host or a longer timeout would let it finish and flip it to `gap_closed` although
nothing was fixed (the final waveform is right).

## Scripts

All need Python 3 with numpy (the ladder oracle is a dense f64 matrix exponential written in
`_noi_common.py`; `python3 _noi_common.py` runs its self-test against closed forms). Options common to all:
`--rustspice BIN` (default `$RUSTSPICE`, else `target/release/rustspice`), `--ngspice BIN` (default
`$NGSPICE`, else `ngspice`), `--prefix "taskset -c 0-1 systemd-run ..."` (command prefix for every child),
`--workdir DIR` (scratch, default a new directory under `/dev/shm`).

| script | gates | exit codes |
|---|---|---|
| `check_card_order_sweep.py [--seeds 8] [--quick] [--slow] [--hang-budget 60]` | RC 5..60 and LC 5..20 ladders against the exact oracle over card-only shuffles (bad = non-finite, exit != 0, or > 1e-3 V RC / 5e-3 V LC); nonlinear controls and the resistor mesh against RustSpice's own natural-order result (> 1e-6 of swing is bad); ngspice invariance is checked as the premise. `--slow` also runs `rob_ord_diode_rc_ladder100_cardshuffle_hang` against the natural original under a time budget. | 0 invariant, 1 at least one violation, 2 premise violated (ngspice itself depends on card order) or tool problem |
| `check_twin_blocks.py [--seeds 8]` | no external reference: in one deck two identical 30-stage ladders, block B shuffled; `max abs(vA-vB)` must stay <= 1e-6 V (ngspice measures ~1e-15). Seeds 1..8 plus the checked-in `rob_ord_twin_rcladder30_blocks`. | 0, 1 violation, 2 tool problem |
| `check_det_threads.py [--big] [--wide]` | determinism anchor: the SHA-256 of the whole rawfile of 5 decks (RC ladder 100, resistor mesh 20, CMOS5, diode mesh 16, resistor mesh 60 on the faer tier) is identical under RAYON_NUM_THREADS 1/4/32, OMP_NUM_THREADS, 1 and 2 cores, repeats. Needs an affinity set of >= 2 cores: run under `taskset -c ...`. `--big` adds a 10,001-unknown mesh (~0.7 GB RSS); `--wide` adds 8 and 16 core configurations (idle host only). | 0 identical, 1 a rawfile differs, 2 tool problem |

## Expected exit codes today

`check_card_order_sweep.py` and `check_twin_blocks.py` exit **1 while the defect exists** (they flip to 0 only
when the numbering dependence is fixed; they are evidence, not regressions). `check_det_threads.py` exits 0.

Run on 2026-10-27 with the frozen `rustspice-p2d`: `check_card_order_sweep.py` exit 1 (30 violations: 28
ladder shuffles, 2 loud diode-mesh failures; `--quick --slow --hang-budget 30 --seeds 2` also exit 1 and the
diode ladder was killed at the budget), `check_twin_blocks.py` exit 1 (6 of 9 decks), `check_det_threads.py`
exit 0 (5 decks x 8 configurations identical).

Typical run, memory < 150 MiB (mesh 60 faer tier 113 MiB), seconds each:

    RUSTSPICE=target/release/rustspice systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=1G \
      timeout 300 python3 benchmarks/checks/numbering_order_invariance/check_twin_blocks.py
