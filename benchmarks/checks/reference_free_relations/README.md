# reference_free_relations

Decks: `benchmarks/cases/accuracy/acc_rel_*` (7 decks, run by `bench-suite`).
Scripts: the five checks below plus the deck generators (not run by `bench-suite`; see `../README.md`).

Idea: properties that must hold exactly whatever the answer is, so a pass does not depend on trusting
ngspice. `bench-suite` has no relation operator, so each relation is evaluated inside the netlist: twin
copies of the circuit and non-loading VCCS cards sense `r = 1 V + K*(relation in volts)`; the reference
value is 1, so the ordinary per-probe gate `abs(v(r) - 1) <= tol.abs` is a gate on the relation residual.
Real node voltages are probed too, so an all-zero solution cannot satisfy a relation. The decks still run
ngspice, so a missing ngspice gives `unverified`, never a reference-free pass.

## Decks

| deck | relation | expected status today |
|---|---|---|
| `acc_rel_reciprocity_randmesh14` | reciprocity `v_j(s_k) = v_k(s_j)`, random 14x14 mesh, UnrolledSymbolic | pass |
| `acc_rel_reciprocity_randmesh22` | same on 22x22, natural numbering: residual 3.3e-9..1.5e-8 against a 1e-9 gate | known_gap_confirmed (`linear-static-pivot-accuracy`) |
| `acc_rel_reciprocity_randmesh22_faer` | tier control: identical relation cards plus one disconnected padding copy move RustSpice to FaerSupernodal, residual <= 4.9e-12; pairs with the gap deck and detects its closure | pass |
| `acc_rel_superposition_randmesh14` | linearity `v(V1+V2+I3) = v(V1)+v(V2)+v(I3)`, UnrolledSymbolic | pass |
| `acc_rel_superposition_randmesh22_faer` | same, 1958 unknowns, FaerSupernodal | pass |
| `acc_rel_tran_superposition_rcladder30` | transient superposition (PULSE + SIN in series vs each alone), three 30-stage ladders | pass |
| `acc_rel_tran_rest_equals_op_cmos5` | DC-driven CMOS chains stay at their `.op` values during `.tran` | pass |

## Scripts

Python 3 with numpy. The independent oracle is a mini deck parser plus a dense float64 MNA solver
(`rfr_common.py`, one long-double refinement step). Binaries: `$RUSTSPICE` (default
`target/release/rustspice`), `$NGSPICE` (default `ngspice`); `RFR_CPUS=16-31` pins every child with
`taskset` (default: no pinning); `RFR_WORK=DIR` sets the scratch directory (default `<tmpfs>/rfr/<name>`).
Every check takes `--selftest`: it injects a known error and exits 0 only if the gate rejects it.

| script | gates (RustSpice only; ngspice is a printed control) | exit codes |
|---|---|---|
| `check_dc_sweep_relations.py` | `.dc` point equals a stand-alone `.op` (diode chain, inverter VTC); up sweep equals down sweep; second difference of an affine linear-mesh sweep is zero (14x14, 22x22 unrolled with a registered known gap, 22x22 padded into the faer tier) | 0 pass, 1 unexpected violation, 3 a known gap no longer reproduces, 4 selftest failed |
| `check_kcl_residual_diode_mesh.py` | independent Shockley KCL recomputation on 4x4, 8x8, 16x16 diode meshes at 3 biases (self-consistency of the solution, not of the diode model; low discrimination by design) | 0, 1, 4 |
| `check_time_scale_invariance.py` | scaling C and all source times by 1e-3, 1e3, 1e6 reproduces the same waveform to 1e-11 V; a 37-row source delay shifts it by exactly 37 rows | 0, 1, 4 |
| `check_tran_convergence_order.py` | Richardson order of the fixed-step transient without any reference: RC, RLC, ladder, gate [1.8, 2.2] | 0, 1, 4 |
| `check_tran_rest_equals_op.py` | `.tran` with DC-only drive equals the RustSpice `.op` for all nodes of the CMOS chains (gate: declared reltol 1e-6 * abs(v) + 1e-10) | 0, 1 (the header also names 2 for setup errors), 4 |

Generators and calibration (reproduce the decks and every number quoted in their notes):

* `gen_dc_decks.py OUTDIR` and `gen_tran_decks.py OUTDIR` regenerate the seven `acc_rel_*` decks;
  the output is byte-identical to `benchmarks/cases/accuracy/acc_rel_*.sp` (compare with `cmp`).
* `calibrate_dc_decks.py [WORKDIR]` and `calibrate_tran_decks.py [DECKDIR [WORKDIR]]` (DECKDIR default
  `benchmarks/cases/accuracy`) print RustSpice, ngspice-tight and the Python oracle residuals per deck,
  the solver tier, and the tier-control variants.

## Expected exit codes today

All five checks exit 0 (including `--selftest`). `check_dc_sweep_relations.py` reports the 22x22 unrolled
mesh as `KNOWN_GAP` and exits 3 if that gap closes (then retag `acc_rel_reciprocity_randmesh22`).
The oracle constant `VT = 0.025864917007 V` was calibrated against the observable output of both tools; the
KCL check verifies self-consistency (KCL), not the diode model.

Run on 2026-10-27 with the frozen `rustspice-p2d`: all five checks and all five `--selftest` runs exit 0;
`gen_*` output identical to the committed decks; `calibrate_*` run.
Memory: <= 2000 unknowns, a few MB each; seconds each.
