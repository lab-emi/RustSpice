# tran_order_and_conservation_oracles

Decks: `benchmarks/cases/accuracy/acc_ord_*` (4 series decks, 22 expanded cases, run by `bench-suite`).
Scripts: the three checks below plus two helper modules (not run by `bench-suite`; see `../README.md`).

The engine can only compare RustSpice with ngspice, and it has no order-of-convergence or conservation
gate (only the nominal member of a series carries `tol.norm`; the coarser members are reported as `info`).
The scripts score RustSpice and the ngspice reference against independent oracles: closed form plus
homogeneous solution (RLC), scalar ODE with the anode eliminated by Lambert W (rectifier), piecewise DOP853
split at the PULSE breakpoints (LC), hand-written level-1 equations (CMOS inverter), integrated with scipy
at rtol 1e-11..1e-13 and cross-checked with a second method.

## Decks

| deck | circuit | nominal step | expected status today |
|---|---|---|---|
| `acc_ord_rlc_sine_series` | series RLC (Q = 2, f0 = 159 MHz), SIN drive | 125p | s4..s6 pass, s1..s3 info |
| `acc_ord_diode_halfwave_rc_series` | half-wave rectifier, 20 periods | 500p | s3..s5 pass, s1..s2 info |
| `acc_ord_lc_free_ring_energy_period` | lossless LC kicked by a 4 ns pulse, then 5.7 free periods | 62.5p | s5..s6 pass, s1..s4 info |
| `acc_ord_cmos_inverter_l1_series` | level-1 CMOS inverter through its switching region | 10p | s3..s5 pass, s1..s2 info |

## Scripts

Python 3 with **numpy and scipy**. Binaries: `$RUSTSPICE` (default `target/release/rustspice`), `$NGSPICE`
(default `ngspice`); `TRANOC_CPUS=16-31` pins every child with `taskset` (default: none); `TRANOC_WORK=DIR`
is the scratch directory (default `<tmpfs>/tranoc/work`). `tranoc_lib.py` (runners, raw reader, deck helpers)
and `tranoc_oracles.py` (the oracles) are imported by the checks and are not meant to be run.
The checks assert exact deck text (SIN and PULSE strings, model parameters), so editing a deck without the
oracle fails loudly.

| script | gates | exit codes |
|---|---|---|
| `check_order_convergence.py [--json F] [stem ...]` | for every series member: G1 two independent oracle integrations agree to 1e-9 of swing; G2 RustSpice error slopes over h <= 2*nominal in [1.7, 2.3], finest pair in [1.9, 2.1], least squares in [1.8, 2.2] (BDF-1 start-up term makes `i(v1)` of the RLC deck approach 2 from below); G3 the suite's ngspice reference mode is >= 20x tighter than `tol.norm` * swing; G4 RustSpice error at the nominal step is inside `tol.norm` (engine and oracle agree) | 0, 1 any violation |
| `check_conservation.py [--json F]` | C1 free-ring energy drift <= 1e-10; C2 sampled ring obeys the trapezoid recurrence and the period error equals the closed-form warping to 1e-8 with slope 2; C3 injected-energy slope in [1.8, 2.2]; C4 discrete energy balance of the driven RLC closes to 1e-9 from step 2 on; C5 first (BDF-1) step defect reported. ngspice is reported only. | 0, 1 |
| `check_existing_series_reference.py` | bounds the ngspice reference error of the existing deck `acc_tr_diode_halfwave_rail` (read only, no oracle in the engine) against the same circuit integrated with scipy | 0, 1 |

## Expected exit codes today

All three exit 0; no registered known gap applies (RustSpice's trapezoid has order 2.00 and conserves energy
to round-off on these decks). The tolerances are 1.35x..2.1x RustSpice's own truncation error at the nominal
step, so a change of the integrator's constant (not only a bug) can move a series member between `pass` and
`fail_accuracy`; the scripts then show which side is right.

Run on 2026-10-27 with the frozen `rustspice-p2d`: `check_existing_series_reference.py` exit 0 (21 s),
`check_conservation.py` exit 0, `check_order_convergence.py` exit 0 (31 s). Memory: tens of MiB.
