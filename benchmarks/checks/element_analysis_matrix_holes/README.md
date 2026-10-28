# element_analysis_matrix_holes

Decks: `benchmarks/cases/capability/cap_xm_*` (12 decks, run by `bench-suite`).
Scripts: the four checks below (not run by `bench-suite`; see `../README.md`).

The element x analysis matrix of the corpus had empty cells: no inductor, VCCS or current source inside a
`.subckt`, no waveform source inside a `.subckt`, no inductor under `.dc`, no VCCS under `.dc`/`.tran`
with a diode. These decks fill them with circuits whose answer is fixed by an independent oracle.

## Decks

| deck | cell | expected status today |
|---|---|---|
| `cap_xm_l_dc_sweep_diode` | L x `.dc`, L as DC short in an R-L-R-diode path | pass |
| `cap_xm_l_dc_sweep_isrc_diode` | L x `.dc` with I1 swept into a node tied to the circuit only through L | pass |
| `cap_xm_g_dc_sweep_diode_load` | G x `.dc`, VCCS into 1k with a diode | pass |
| `cap_xm_g_tran_sine_diode_rc` | G x `.tran`, VCCS from a 10 MHz sine into RC with a diode clamp | pass |
| `cap_xm_i_pulse_diode_rc_tran` | I (PULSE) x `.tran` with a diode branch | pass |
| `cap_xm_l_tran_diode_decay` | L x `.tran`, inductor current commutates into a diode loop | pass |
| `cap_xm_sub_dc_sweep_diode_divider` | `.subckt` (R + diode) twice, `.dc`, internal nodes `x1.k`, `x2.k` probed | pass |
| `cap_xm_sub_g_internal_ctrl_dc_sweep` | VCCS inside a `.subckt` controlled by an internal node, `.dc` | pass |
| `cap_xm_sub_cmos_chain4_tran` | 4-stage CMOS chain from one `.subckt inv`, `.tran` | pass |
| `cap_xm_sub_rlc_ladder_tran` | RLC ladder from two nested `.subckt` levels | pass |
| `cap_xm_sub_source_pulse_sin_tran` | PULSE V and SIN I sources declared inside a `.subckt` | pass |
| `cap_xm_sub_instance_name_waveform_tran` | SIN source in a `.subckt` instantiated as `Xpulse1` (control: same `.subckt` as `X2`) | pass (closed 2026-10-28) |

The gap: RustSpice flattens an instance into names like `i1_xpulse1` and looks for the waveform keyword as a
substring of the whole card, name included, PULSE before SIN, so a valid `SIN(0 1m 10meg 0)` card is read as
PULSE and `v(o1)` stays 0 V; renaming the instance flips the result (`check_instname_trigger.py`).

The two `.dc` inductor decks report a case-level oracle status `unsupported` (the engine oracle
`dc_nonlinear` cannot evaluate `i(l1)`); the per-probe numbers are present and `i(l1)` is covered by
`check_independent_oracles.py`. Probes of inductor currents inside a `.subckt` cannot be compared by the
engine (RustSpice names `i(l1_x1.x1)`, ngspice `i(l.x1.x1.l1)`); only node voltages are probed there.

## Scripts

Python 3 with numpy; `check_independent_oracles.py` also needs **scipy and mpmath**. Binaries:
`$RUSTSPICE` (default `target/release/rustspice`), `$NGSPICE` (default `ngspice`); `EAMH_CPUS=16-17`
pins every child with `taskset` (default: none). Memory: a few MiB to a few tens of MiB.

| script | gates | exit codes |
|---|---|---|
| `check_independent_oracles.py [--deck SUBSTR] [--verbose]` | all 12 decks against hand-written oracles that share no code with either simulator: 50-digit mpmath MNA + Newton for the DC decks, scipy Radau (rtol 1e-12) or exact matrix exponentials for the transient decks; RustSpice within the deck tolerance, the ngspice reference within tolerance/4, mutation teeth (scaling a diode or MOS strength leaves the band), and on the gap deck RustSpice wrong on exactly `v(o1)` and right on `v(o2)` | 0 every audit line OK, 1 violation, 2 tool missing |
| `check_dc_sweep_vs_op.py` | path independence of the five `.dc` decks in RustSpice and tight ngspice: every sweep point equals a stand-alone `.op` with the source set to that value, and the backward sweep equals the forward sweep (threshold: the deck's own band) | 0, 1, 2 |
| `check_hierarchy_flat_equivalence.py` | each `.subckt` deck equals its hand-flattened twin to 1e-9 of swing, in both tools; for the instance-name deck it documents the gap (RustSpice hierarchical != flat on `v(o1)`, ngspice equal) | 0, 1, 2 |
| `check_instname_trigger.py` | 9 variants prove the substring mechanism: SIN in an instance named `...pulse...` is wrong, PULSE in an instance named `...sin...` is right, renaming flips it, ngspice right everywhere | 0 prediction holds, 1 violated, 2 |

## Expected exit codes today

All four exit 0 (the documented gap is part of what they assert). A script flips to a failure when the
gap is fixed, which is the cue to retag `cap_xm_sub_instance_name_waveform_tran` (`gap_closed`).

The oracle constants k = 1.38064852e-23, q = 1.6021766208e-19, T = 300.15 K were re-derived from the
junction voltage ngspice-47 prints; with CODATA-2019 constants every diode deck would show ~2e-7 V offsets in
all three tools. The `.dc` decks use tol.rel 1e-8 / tol.abs 1e-12 (2.7..7.7 % of the band consumed), so a
different ngspice build with a different convergence floor could consume more margin.
`cap_xm_sub_dc_sweep_diode_divider` depends on the shared 1e-12 S junction gmin convention below ~1 V.

Run on 2026-10-27 with the frozen `rustspice-p2d`: `check_independent_oracles.py` exit 0 (34 s),
`check_dc_sweep_vs_op.py` exit 0, `check_hierarchy_flat_equivalence.py` exit 0,
`check_instname_trigger.py` exit 0.
