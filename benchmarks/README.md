# RustSpice vs ngspice: differential benchmark suite

`rustspice bench-suite` runs every deck in [`cases/`](cases) through **RustSpice** and through the black-box executable
**ngspice-47** (`ngspice` on `PATH`), compares the results, and reports one verdict per case. It is built to answer three
questions with evidence rather than with a headline ratio:

* **Capability** – does RustSpice accept and correctly evaluate what ngspice accepts (elements, analyses, syntax, models)?
* **Accuracy** – are its numbers right, judged against a *tightly converged* ngspice and, where one exists (209 of the 951 static
  cases), an independent oracle (closed form, dense f64 solve, matrix exponential), not against ngspice's default-tolerance output?
* **Robustness and performance** – does it refuse hostile input cleanly, stay bounded in memory and time, and how does the
  end-to-end wall time (process start to rawfile) compare, on which circuit families, at which sizes?

ngspice is used **only as an executable** (AGENTS.md Rule 1): its output files and timings are observed, its source is never read.
A missing or broken ngspice yields `unverified`, never a pass.

```sh
cargo build --release -p rustspice-cli
# correctness only, no timing, no fuzz (about 3-4 minutes on the reference host: all static decks, the small generated families, the mutation check)
./target/release/rustspice bench-suite --no-timing --no-fuzz --scale small --teeth --report bench.md --json bench.json
# one group / one deck family, timed, pinned to one core (pick an idle core: see "Reading timing results")
./target/release/rustspice bench-suite --group performance --reps 9 --pin-cpu 11 --report perf.md
# the six legacy head-to-head benchmarks (honest rewrite of the old `bench-ngspice`)
./target/release/rustspice bench-ngspice --reps 9 --pin-cpu 11 --output docs/benchmark_report.md
```

Exit code: `0` = no failing case (known gaps and `unverified` cases are allowed; they are listed in the verdict line and the report),
`1` = at least one `fail_*`/`panic`/`timeout`, `2` = usage or environment error (a `--cases` directory that is missing or holds no deck,
ngspice missing or unable to simulate a start-up self-test without `--allow-unverified`, invalid numeric arguments, a harness I/O
failure such as a full scratch disk, or any `unverified` case under `--strict-unverified`).

Layout: [`cases/`](cases) the decks, one directory per group; [`checks/`](checks) scripts for what a differential run cannot express;
[`proposed-fixes/`](proposed-fixes) historical root-cause patch write-ups (now integrated into the crate sources); [`SUITE_SPEC.md`](SUITE_SPEC.md) the design contract that the engine's
diagnostics and the decks' notes cite (`SUITE_SPEC 13.1`).

## What is in the corpus

Reference run `--no-timing --no-fuzz --scale small --teeth` (951 cases; the 23 generated cases of the small scale are counted in `performance`):

| group | deck files | cases | pass | known gap | info | unverified | what it asks |
|---|---|---|---|---|---|---|---|
| capability | 378 | 378 | 314 | 49 | 15 | 0 | Does RustSpice accept and correctly evaluate what ngspice accepts? `cap_el` elements, values and sources (72); `cap_dev` device models and parameters, diode and MOS levels (124); `cap_lang` netlist language, directives, analyses, `.subckt`, `.param` (163); `cap_xm` element x analysis cross-matrix holes (12); `cap_cli` fixtures for the CLI surface (7). |
| accuracy | 157 | 207 | 181 | 6 | 20 | 0 | Is the number right, against converged ngspice and an independent oracle? `acc_dc` operating points and sweeps (84); `acc_tr` transients: step, edge, ringing, pulse alignment (62 decks, 94 cases with their series members); `acc_rel` reference-free relations: reciprocity, superposition, rest equals OP (7); `acc_ord` order of convergence (4 series decks, 22 cases). |
| robustness | 295 | 295 | 253 | 0 | 42 | 0 | Does it refuse hostile input cleanly and stay bounded? `rob_hs` hostile, degenerate and resource-bomb input (208); `rob_cv` convergence-hard circuits: rectifiers, rings, OTAs, deep chains (75); `rob_ord` card-order and numbering invariance (12). |
| performance | 48 | 71 | 70 | 0 | 0 | 1 | How fast, on which circuit class and at which size? `perf_dc/lin/med/nl/out/su` hand-built timing decks (36), `perf_scal` memory and scaling families (12); plus the generated families below. |

`unverified` is the remaining `faer-nonlinear-slow` static deck (`perf_med_diode_mesh_op_32x32`) that needs timing to be judged (`--no-timing` skips it). A static case is one deck or one member of a `bench.series.tstep` series (61 members). The generated part is 20 families at `--scale small`
(more and larger at `medium` and `large`): resistor and RC ladders, 2-D meshes (also with shuffled numbering) and RC meshes, LC ladders, diode chains and meshes, CMOS chains, MOS arrays, 6T SRAM arrays, star hubs, parse-stress decks and a start-up floor. The seeded fuzzers (`--fuzz-count`, `--seed`) produce random linear, RC, diode and MOS decks and structure-preserving mutants of the static decks.

Decks are plain SPICE with `* bench.*` comment lines (identical text is fed to both simulators; see "Deck metadata"). Generated
families (`fam_*`: scale ladders per circuit class) and seeded fuzzers/mutators are produced in Rust (`--scale small|medium|large`,
`--seed`, `--fuzz-count`). Decks that cannot carry comments (empty file, whitespace only, UTF-16) have a sidecar `<deck>.sp.meta`.

`checks/` holds self-consistency and invariance scripts that the differential engine cannot express (card-order invariance,
determinism across thread counts, reciprocity/superposition, order of convergence, adjoint/batch/backend/codegen surface).
`bench-suite` does not run them; see `checks/README.md`. `proposed-fixes/` holds the root-cause patch proposals and single-patch measurements from the initial audit (all seven root-cause patches have now been integrated into the crate sources).

## Verdicts (`status`)

| status | meaning |
|---|---|
| `pass` | RustSpice meets `bench.expect` (default `match`: reproduces the reference within the case tolerance). |
| `fail_accuracy` / `fail_robustness` / `fail_silent` | A deck **without** a known-gap tag violates its expectation (numbers differ / RustSpice refuses what ngspice solves / RustSpice produced numbers where refusing was required). Fails the run. |
| `known_gap_confirmed` | The deck carries `bench.known_gap: <slug>` and RustSpice still violates the expectation. Does not fail the run; listed by slug. |
| `gap_closed` | A tagged gap no longer reproduces: remove the tag (and update the docs). |
| `panic` / `timeout` | RustSpice aborted, crashed, ran away or was killed at the memory cap. Always a failure, except under the two resource slugs that document it (`resource-budget`, `symbolic-lu-blowup`) and, for a timeout where ngspice finished, under the performance slug `faer-nonlinear-slow`. |
| `both_error` / `ngspice_error` | ngspice itself failed or crashed (the case is not judged against it). |
| `skipped_budget` | Estimated memory above `--max-rss-mib` (Rule 9). |
| `unverified` | Nothing could be judged (ngspice missing, unresolvable reference, invalid metadata). Never a pass. |
| `info` | `expect: either`: neither tool is provably right (ambiguous or ngspice-specific behaviour); recorded, not judged. |

`outcome` (JSON) is the independent observation (`match | mismatch | loud_error | silent_numbers | panic | timeout |
ngspice_only_error | both_error`); the capability matrix is drawn from it. Unmeasured quantities are `null` plus a `*_status`
string (Rule 4), never `0`.

## Honest-comparison rules

1. A reference must be trustworthy: tight ngspice (`.options reltol=1e-9 vntol=1e-12 abstol=1e-15`; transients: `reltol=1e-6`,
   `tmax = tstep/20`) and, where one exists, agreement with an independent oracle. ngspice at default tolerance is itself 1e-4..1e-5 V
   off, so RustSpice is compared with the converged reference and with ngspice's own as-is error side by side.
2. Tolerances are never loosened to make a deck pass unless the note proves the loosened level is the reference's own accuracy.
3. A gap is confirmed when RustSpice violates the expectation against the tight reference; the deck's `bench.note` records the independent
   evidence (closed form, hand calculation or the in-engine oracle, which also checks 40 of the 338 confirmed cases). The 330 decks that carried a gap tag
   at the time of that review were re-verified by a second, independent team that tried to refute each claim (283 confirmed as stated; 34 had a
   better slug, which was corrected; 9 ambiguous and 2 not defects were turned into `info`; 2 deck bugs fixed).
4. `loud_error` is expected only where refusing is *correct* (hostile or invalid input, input ngspice also rejects, resource
   bombs). A feature that ngspice computes and RustSpice refuses is `expect: match` + `unsupported-*-loud`, so the case flips to
   `gap_closed` when it is implemented.
5. `--teeth` re-runs RustSpice alone on decks with one of three elements scaled by 5 % and compares with the original ngspice data:
   a pass that survives every mutation is reported as `teeth: blunt` (the comparison could not see the circuit).
6. The suite reports defects; it never changes RustSpice or ngspice behaviour.

## Timing methodology

Both contenders are **subprocesses** writing a binary rawfile to tmpfs (`ngspice -b -r`, `rustspice run --raw --stats`). They pay
process start-up, parse, setup, solve and output. Interleaved A/B repetitions in rotating order after one warm-up (the correctness
run), `--reps` >= 5, min / median / stddev, `noisy` = stddev/median > 0.2, load average before and after. ngspice is timed with **both**
of its direct solvers: SPARSE 1.3 (its default) and KLU (`.options klu`); the headline compares with the faster one.
Headline = end-to-end median ratio. *Solve-only* uses ngspice `Total analysis time` against RustSpice `analysis_us`, which
**excludes RustSpice's compile time** (symbolic LU analysis, JIT): it is reported for diagnosis only and is never the headline.
Start-up floors (an empty deck: about 2.4 ms for ngspice and 0.8 ms for RustSpice in the reference run, they move with load) are measured once per run and printed.
Peak RSS is `VmHWM` of each child polled from `/proc` (never `wait4 ru_maxrss`, which is floored by the harness's own high-water
mark); RustSpice's own `--stats` reading is merged in. ngspice's poll-based peak is a lower bound (`rss_lower_bound` in the JSON).
Fewer than three samples are never reported as stable (`spread_status`, `noisy: true`).

**Reading timing results:** the reference host is a hybrid 24-core i9-13900HX shared with unrelated jobs (load average 1.5-28 during the reference runs, up to about 66 earlier in the session). Absolute
numbers are valid for one host and one session only; trust ratios within one report, and prefer an idle P-core for `--pin-cpu`
(`/proc/stat` over 3 s). Reports state the load before/after and flag noisy cases.

Performance decks only *fail* on accuracy. Speed is judged where a slug says so: `faer-nonlinear-slow` is judged on the
end-to-end median (gap closed only at >= 1.1x and not noisy). Every other timed case that is slower than ngspice is listed in the
report's "Slower than ngspice" table, so performance weaknesses that do not break accuracy stay visible.

## Deck metadata (`* bench.<key>: <value>`)

`id` (= file stem, `[a-z0-9_-]+`), `group` (capability|accuracy|robustness|performance), `title`, `probes` (comma list, ngspice
names; absent: every `v()` in both outputs), `tol.rel`/`tol.abs` (op/dc: `|a-b| <= abs + rel*|ref|`), `tol.norm` + `floor` (tran:
`max|a-b|/max(swing, floor)`), `expect` (`match`|`loud_error`|`either`), `known_gap` + `gap.note` (one line: what RustSpice does
today), `ng.options`, `ng.solver` (`sparse`|`klu`|`both`), `ref` (`tight`|`none`|options) and `ref.tmax`, `oracle`
(`dc_linear`, `dc_nonlinear[(gmin=..)]`, `tran_lti`, `ladder(n,rs,rsh)`), `series.tstep` (work-precision series, one case per
value `__s<k>`), `size`, `tags`, `timing`, `est_rss_mib` (RustSpice peak estimate, Rule 9), `note`.

Tags with engine semantics: `ngspice_error_ok` (ngspice may print singular-matrix warnings and still write a rawfile),
`ngspice_unbounded` (ngspice is not run; judged against `expect` only), `npoints` (also compare row counts), `timing_edge`
(report 50 %-crossing shift), `ngspice_crash` (ngspice-47 itself crashes), `caught_panic_noise` (RustSpice prints a caught faer
panic and still exits 1: cosmetic), `lenient_ok` (informational), `family:<id>`. All other tags only group and filter.
A `known_gap` together with `expect: either` is invalid (an `either` deck can only violate through a crash), except for `resource-budget`; a gap without `bench.gap.note` is invalid; the performance slug needs `expect: match`. Invalid metadata is a diagnostic and the case is `unverified`.

## Known-gap registry (`bench.known_gap`)

Counts are cases tagged with the slug in the reference static run of 2026-10-28 (after the plan's Phases 1-3: adaptive transient, strict reader, expressions, temperature, includes, `.dc` sweeps, then the diode / level-1 MOS parameters and the controlled and coupled elements): 17 cases, of which 16 are `known_gap_confirmed` and 1 is the timing-judged `faer-nonlinear-slow` static deck (`perf_med_diode_mesh_op_32x32`, unjudged without timing). Phase 3 closed 39 (31 `silent-ignore-model-param`, 7 `unsupported-device-loud`, 1 `silent-misparse`). A deck names exactly one slug; `bench.known_gap` with any other string is rejected. Earlier closures came from the seven root-cause patches (`RC-lu-pivot`, `RC-time-grid`, `RC-newton-clamp`, `RC-newton-limit`, `RC-faer-slow`, `RC-resource-budget`, `RC-parser-silent`; see `proposed-fixes/README.md` and `SUITE_SPEC.md` 15.1).

| slug | cases | status / what RustSpice does |
|---|---|---|
| `silent-ignore-directive` | 1 | `.if/.elseif/.else/.endif` blocks are refused with an error (ngspice evaluates them). **Closed 2026-10-28**: `.include`/`.inc`/`.lib <file> <section>` are read (nesting, quoted names, sections; cycles, missing files and missing sections are refused), `.global`, `.nodeset` (two-phase operating point), `.options gmin` (junction gmin), `.model` inside `.subckt`; undefined, mistyped and wrong-type models are refused. |
| `silent-ignore-analysis` | 0 | **Closed 2026-10-28**: `.ac/.tf/.noise/.sens/.pz/.disto/.step` refuse the deck; a `.control` block with a command that changes the circuit or the analyses (`alter`, `let`, `tran`, ...) is refused, one with only `run`/`write`/`print`/`plot`-style commands is skipped with a warning; `.meas/.four` still warn. |
| `silent-ignore-model-param` | 0 | **Closed 2026-10-28** (Phase 3): diode RS (internal node), BV/IBV (the breakdown knee is fitted so that i(-BV) = -IBV), IKF, ISR/NR, TT and the graded CJO/VJ/M/FC depletion charge; MOS level 1 derives KP from UO/TOX, PHI/GAMMA/VTO from NSUB/NSS/TPG (eps_ox = 3.9 x 8.854214871e-12 F/m), shortens L by 2 LD, adds RD/RS (or RSH x NRD/NRS) as internal nodes, and models the bulk junctions (IS/JS, CBD/CBS or CJ/CJSW with AD/AS/PD/PS, MJ/MJSW/PB/FC) and Meyer gate capacitances. Parameters that are still not modelled (noise KF/AF, ...) warn. |
| `silent-wrong-model-level` | 4 | MOS `LEVEL>=2` is refused with an error instead of being evaluated as another model; the 4 remaining decks (LEVEL 2, 3, 4, 6) expect ngspice's numbers. The 4 BSIM-number decks that expect a refusal are closed. |
| `silent-ignore-source-waveform` | 0 | **Closed 2026-10-28**: sources are parsed by keyword, not by name substring; PULSE (with or without parentheses, SPICE defaults), SIN with phase, EXP, PWL (`r=`, `td=`), SFFM are evaluated with their breakpoints; AM/TRNOISE/TRRANDOM and malformed lists are refused. |
| `silent-ignore-temperature` | 0 | **Closed 2026-10-28**: `.temp`, `.options temp/tnom`, instance `temp/dtemp`, `tc1/tc2`, model `TNOM/EG/XTI`; diode IS/VJ/CJO and MOS level-1 KP/VTO/PHI follow the textbook temperature laws. Only `tce` and multi-value `.temp` still warn. |
| `silent-ignore-multiplier` | 0 | **Closed 2026-10-28**: instance `m=` scales R, C, L, G, D (area) and M (width); positional diode area is honoured. |
| `silent-ignore-ic` | 0 | **Closed 2026-10-28** (Phase 1): `uic`, element `ic=`, `.tran tstart/tmax` and the spaced `.ic V(x) = v` are applied. |
| `expr-truncation` | 0 | **Closed 2026-10-28**: recursive-descent `.param`/`{...}`/`'...'` evaluator (precedence, unit suffixes, built-in functions, `.func`, ternary, chained parameters with cycle detection, `.subckt ... PARAMS:` overrides); an expression that cannot be evaluated is refused. |
| `dc-clobbers-waveform` | 0 | **Closed**: `.dc` sweep now saves and restores the swept source's DC value and waveform after the sweep completes. |
| `clamp-rail` | 0 | **Closed** (`RC-newton-clamp`): replaced arbitrary rail-box clamping with junction voltage limiting (`pnjlim`/`fetlim`), source stepping, and transient pseudo-transient/sub-stepping recovery. |
| `false-convergence-pnjlim` | 0 | **Closed** (`RC-newton-limit`): Newton requires both `|dv|` and `|pnjlim_Correction|` to satisfy tolerances before declaring convergence. |
| `stale-lu-after-faer-fallback` | 0 | **Closed** (`RC-lu-pivot`): `factors_in_faer` / `factors_dense` flags and clearing `ordering`/`unrolled_jit` prevent stale LU buffer reuse after demotion. |
| `no-op-homotopy-tran` | 0 | **Closed** (`RC-lu-pivot`, `RC-newton-clamp`): transient initial operating point uses adaptive Gmin/source-stepping homotopy and stable pivoting. |
| `dc-sweep-no-recovery` | 0 | **Closed** (`RC-lu-pivot`, `RC-newton-clamp`): DC sweep points fall back to adaptive Gmin and source-stepping homotopy on non-convergence. |
| `fixed-step-accuracy` | 0 | **Closed 2026-10-28** (Phase 1): LTE step control (divided differences, trapezoid / BDF-2 / BE), waveform breakpoints with a BE-BE-BDF2-trapezoid restart, linear circuits on an `hmax/2^k` step ladder with cached factors. |
| `abstol-absolute-residual` | 0 | **Closed** (`RC-newton-limit`): KCL residual check scales with `reltol * max_branch_current + abstol`. |
| `resource-budget` | 0 | **Closed** (`RC-resource-budget`): explicit `ResourceLimits` caps on element counts, subcircuit expansion depth, sweep points, transient steps, and non-finite parameters. |
| `symbolic-lu-blowup` | 0 | **Closed** (`RC-lu-pivot`, `RC-faer-slow`): symmetric minimum-degree ordering with work budget (`200_000`) and symbolic LU operation cap (`32 * dim` for `dim >= 256`, `500_000` max) route high-fill topologies directly to cached `faer` sparse LU. |
| `faer-nonlinear-slow` | 1 | Nonlinear 2-D arrays/meshes on the `faer` tier (`perf_med_diode_mesh_op_32x32` static deck + `fam_mos_array_tran_fast_5` generated deck; `perf_med_mos_array_op_20x20` closed at 1.18x faster). |
| `r-nonpositive-clamp` | 0 | **Closed** (`RC-resource-budget`): `R < 0` is rejected with a topology error; `R = 0` uses a `1e-9` ohm substitute conductance while rejecting zero-resistance topology loops cleanly. |
| `linear-static-pivot-accuracy` | 0 | **Closed** (`RC-lu-pivot`): diagonal-preferring threshold Markowitz pivoting (`0.1`), stability growth check, and automatic fallback to `DensePartialPivotLu` / `FaerSupernodal`. |
| `batch-all-or-nothing` | 0 | `run_batch_dc_op` reuses the pre-compiled symbolic LU structure across Rayon workers (`linear_solver.clone()`). |
| `adjoint-coverage` | 0 | CLI `adjoint` outputs all sensitivities (no 15-row truncation). |
| `unsupported-syntax-loud` | 0 | **Closed 2026-10-28**: `.param` expressions, `.subckt PARAMS`, `.dc` over a resistor or `temp`, nested `.dc`, non-UTF-8 bytes in the title and comments. |
| `unsupported-device-loud` | 11 | Device cards B S W T O Q J Z U ... and `.model` cards of BJT/JFET/MESFET types are refused with an error. **Closed 2026-10-28** (Phase 3): E, F, H (linear; E and G also `POLY(1)`), K (mutual inductance, 0 < \|k\| <= 1) and the semiconductor `.model R` / `.model C` (RSH, CJ/CJSW, NARROW/SHORT, DEFW). `POLY(n>1)` and F/H `POLY` are refused. |
| `lenient-malformed-input` | 0 | **Closed 2026-10-28**: input that ngspice rejects is refused with a parse error (unknown cards and instance parameters, unparsable values, extra tokens, duplicate element names, unterminated `.subckt`, empty or malformed waveform lists, `.ic` with unparsable values). |
| `silent-misparse` | 0 | **Closed 2026-10-28**: nested `.dc`, `dc=3.3`, `G ... POLY(1)` (evaluated). |
| `dc-value-ignored-with-waveform` | 0 | **Closed 2026-10-28**: an explicit `DC <v>` next to a waveform is the operating-point value. |
| `dc-static-homotopy` | 0 | **Closed** (`RC-newton-clamp`): adaptive Gmin stepping and source-stepping homotopy solve high-gain feedback circuits (OTA follower). |
| `exp-argument-clamp` | 0 | **Closed** (`RC-newton-limit`): diode exponential linearizes smoothly above `VD_LINEARIZE = 40 * N * Vt` (`exp(40)`) with continuous value and derivative instead of saturating at 80. |
| `ill-posed-dc-silent` | 0 | Registered; the decks are `expect: either` because ngspice gives no truth either. |
| `title-line-parsed-as-card` | 0 | **Closed 2026-10-28**: physical line 1 is always the title. |
| `tran-degenerate-last-step` | 0 | **Closed** (`RC-time-grid`): integer-counted transient grid (`k * tstep`) merges sub-tolerance tails into the final interval stamped `tstop`, eliminating sliver steps. |
| `dc-sweep-grid-drift` | 0 | **Closed** (`RC-time-grid`): integer-counted sweep length `floor(|stop - start| / |step| + tol_q) + 1` eliminates dropped endpoints. |
| `unrolled-lu-tran-divergence` | 0 | **Closed** (`RC-lu-pivot`): transient companion matrix triggers `refresh_for_transient` and runtime factor-growth monitoring with fallback to `DensePartialPivotLu` / `FaerSupernodal`. |
| `nonfinite-result-accepted` | 0 | **Closed** (`RC-resource-budget`): non-finite element/source values and non-finite solution vectors are rejected with an error. |
| `step-retry-grid-desync` | 0 | **Closed** (`RC-time-grid`): Newton step-halving retries split only the failed interval and resynchronize onto `k * tstep`. |
| `newton-iteration-cap` | 0 | **Closed** (`RC-newton-clamp`): raised the Newton iteration budget and added source-stepping / Gmin homotopy so 400-stage CMOS chains converge. |
| `param-floor-clamp` | 0 | **Closed** (`RC-newton-limit`): removed artificial floors on positive MOS `PHI` (`0.1`) and `L`/`W` (`1e-12`). |

## Reference results

**Static correctness run** (`--no-timing --no-fuzz --scale small --teeth --case-timeout 30 --max-rss-mib 2048`, ngspice-47, RustSpice of 2026-10-28 after Phase 3): 951 cases, verdict **PASS**: **857 pass** (818 after Phases 1-2, 635 before), **16 known gaps confirmed** (55 after Phases 1-2, 237 before; Phase 3 closed 31 `silent-ignore-model-param`, 7 `unsupported-device-loud` and 1 `silent-misparse`), **77 informational**, **1 unverified** without timing (`perf_med_diode_mesh_op_32x32`), **0 failing**, 0 `gap_closed`. Capability group: 347 / 378 pass. Mutation check (`--teeth`): **701 passing cases detect a 5 % change of one element**, **0 are blunt** (92 of them see it only through a source), 26 skipped and 224 not applicable. Remaining gaps: `unsupported-device-loud` 11 (BJT x3, JFET, MESFET, B x2, O, S, T, W), `silent-wrong-model-level` 4 (MOS LEVEL 2/3/4/6, refused) and `.if` blocks (refused).

**Timed performance group** (`--group performance --no-fuzz --reps 9 --pin-cpu 11 --case-timeout 60 --max-rss-mib 2048`, scale `medium`): 113 cases, 110 timed, verdict **PASS** (111 pass, 2 known gaps: `fam_mos_array_tran_fast_5` and `perf_med_diode_mesh_op_32x32`; `perf_med_mos_array_op_20x20` closed at 1.18x faster). Host: 13th Gen Intel(R) Core(TM) i9-13900HX, both tools pinned to CPU 11, tmpfs scratch. Start-up floors (empty deck): ngspice 3.04 ms, RustSpice 0.99 ms. After Phase 3 (per-transistor bulk junctions and Meyer capacitances) the group was re-timed at scale `small` (`--reps 5 --pin-cpu 5`, 71 cases, 69 timed): verdict **PASS** (70 pass, 1 known gap `perf_med_diode_mesh_op_32x32` at 0.73x), RustSpice faster end to end in 67 of 69 (geometric mean 2.95x); the nonlinear MOS/SRAM cases stay faster (`fam_sram6t_tran_8` 1.80x, `fam_sram6t_op_8` 2.21x, `perf_med_cmos_chain_tran_50` 2.62x, `perf_med_mos_array_op_20x20` 1.24x) except `fam_mos_array_tran_20`, which remains 5.5x slower (5.4x in the medium run above).

End-to-end (process start to rawfile written), ratio = faster ngspice solver / RustSpice, medians of 9 interleaved repetitions: RustSpice faster in **101 of 110** cases (up from 90 of 108 pre-fix), geometric mean **2.51x** (up from 2.06x pre-fix).

| unknowns | cases | RustSpice faster | geometric mean | range |
|---|---|---|---|---|
| <= 100 | 34 | 34 | 3.48x | 1.08x .. 12.0x |
| 101-1,000 | 35 | 34 | 2.42x | 0.93x .. 6.5x |
| 1,001-5,000 | 21 | 13 | 1.06x | 0.04x .. 9.3x |
| > 5,000 | 20 | 20 | 3.84x | 2.04x .. 19.3x |

Fastest: `perf_scal_rc_mesh_tran_g14` 19.3x, `fam_rc_mesh_tran_14` 18.6x, `perf_lin_rlc_pulse_1e5` 12.0x, `perf_lin_rc_sine_1e6` 12.0x, `perf_lin_lc_tank_sine_1e5` 11.4x, `perf_lin_butter5_step_1e5` 10.6x. All 2-D and 3-D linear meshes (`fam_mesh_dc_100` 2.05x faster, `fam_mesh_dc_shuffled_40` 2.13x faster, `perf_scal_mesh3d_dc_n15` 2.07x faster) and LC ladders (`fam_lc_ladder_tran_300` 2.26x faster) that were 3x..37.5x slower before `RC-faer-slow` are now faster than ngspice-KLU. Only 9 nonlinear 2-D MOS/SRAM/diode array/mesh cases remain slower than ngspice-KLU (every one of them passes its accuracy check):

| deck | unknowns | RustSpice | vs |
|---|---|---|---|
| `fam_sram6t_tran_24` | 1154 | 24.4x slower | klu |
| `fam_mos_array_tran_30` | 2704 | 14.7x slower | klu |
| `fam_mos_array_tran_20` | 1204 | 5.4x slower | klu |
| `fam_diode_mesh_48` | 2306 | 2.2x slower | klu |
| `fam_sram6t_op_24` | 1154 | 1.8x slower | klu |
| `fam_mos_array_op_30` | 2704 | 1.7x slower | klu |
| `perf_med_diode_mesh_op_32x32` | 1026 | 1.4x slower | klu |
| `fam_diode_mesh_16` | 258 | 1.1x slower | klu |
| `fam_mos_array_op_20` | 1204 | 1.03x slower | klu |

**The six legacy benchmarks** (`bench-ngspice`, same circuits and sizes as the old report, rewritten as subprocess-to-subprocess timing with full-waveform accuracy against an independent oracle). Speedup = ngspice (faster of SPARSE and KLU) / RustSpice, end to end, median:

| benchmark | RustSpice | ngspice | speedup | max error (RustSpice / ngspice default) |
|---|---|---|---|---|
| b1 DC ladder | 11.13 ms | 25.98 ms | **2.33x** | 1.91e-13 / 1.93e-13 V |
| b2 RC line tran | 31.45 ms | 114 ms | **3.62x** (8.2x lower error than ngspice default) | 2.01e-4 / 1.65e-3 of swing, against tight ngspice (no oracle) |
| b3 diode sweep | 3.35 ms | 6.54 ms | **1.95x** | 5.89e-11 / 5.44e-5 V |
| b4 CMOS chain tran | 32.12 ms | 35.60 ms | **1.11x** (5.2x lower error than ngspice default) | 7.40e-2 / 3.86e-1 of swing, against tight ngspice (no oracle) |
| b5 sensitivity | 1.30 ms | 12.55 ms | **9.66x** (**306x** vs FD loop) | 9.33e-15 / 1.00e-6 |
| b6 batch | 3.46 ms | 4.30 ms | **1.24x** (min **1.33x**; **2.57x** via `.dc` sweep) | 2.47e-11 / 1.63e-5 V |

Compare the previously published 5.3x / 7.6x / 3.0x / 2.6x / 6285x / 321x: those came from the harness errors listed under "What the suite found about its own first version". On b2, the single short backward-Euler restart at fast slope corners and the corrected jump detector reduced RustSpice's error from `4.61e-3` (2.80x ngspice default) to `2.01e-4` (8.2x below ngspice default's `1.65e-3`) at `tstep = 10 ps`, and on the step ladder `b2s_rcline_500` (`tstep` 10 ps..1 ps) RustSpice is both 3.4x–4.2x faster and 2.7x–15x more accurate than ngspice default (no equal-error comparison exists: the error ranges do not overlap). The b2 50 % crossing shift read from the rawfiles (`2.00 ps` RustSpice, `0.36 ps` ngspice default) measures the output rows, not the solver: RustSpice writes only the 10 ps `k * tstep` rows and ngspice its internal points, and the tight reference itself, read only at RustSpice's rows, is off by `2.01 ps`. The report therefore also gives the shift with the reference read at each tool's own rows (`0.07 ps` RustSpice, `0.35 ps` ngspice) and that row floor; `rustspice run --internal-points` writes the accepted steps between the rows as well (0.14 ps from the rawfile); on b4, the quadratic predictor, contraction-rate Newton acceptance and jagged Jacobian gather cut RustSpice's analysis time from 47 ms to 26 ms (including the bulk junctions and Meyer capacitances of Phase 3) at an unchanged `7.40e-2` error (1.60 ps crossing shift, 5.2x below ngspice default's `3.86e-1` / 9.10 ps shift); b5 compares one adjoint solve with the best ngspice alternative (`.sens`, which computes 24 sensitivities per requested gradient; **306x** against the finite-difference loop that delivers only the 200 requested ones); b6 is a parallel workload (32 threads) against ngspice's 1-thread `.dc` sweep (**1.24x** with `rustspice-batch`, **2.57x** with `rustspice-dc-sweep`, **6.80x** against `ngspice-control-loop-sparse`, and **12.2x** against `ngspice-par-launches`). Batched DC operating points at 1e4..1e6 corners on an NVIDIA GPU (the b6 diode cascade and the b4 CMOS inverter chain) are timed separately by `rustspice bench-batch` ([`docs/gpu_batch_report.md`](../docs/gpu_batch_report.md)): ngspice has no batched or GPU counterpart, so it compares the CPU batch API, the kernel's own CPU build and the CUDA run.

**Seeded fuzzers and mutants** (`--scale small --fuzz-count 100 --seed 2`, 400 random decks and 200 mutants):
- **Random decks (`fuzz_`)**: **400 / 400 pass**, 0 known gap, 0 failing (all `symbolic-lu-blowup`, `no-op-homotopy-tran`, `abstol-absolute-residual`, and `clamp-rail` failures on random linear, RC, diode, and MOS decks are closed).
- **Mutants (`mut_`)**: **200 / 200 informational**, 0 known gap, **0 failing / 0 panics** (all 22–24 resource-bomb aborts and both seed-2 `80 GB` allocation panics `mut_builtin_rc_badutf_52` and `mut_acc_tr_buck_converter_fine_nul_141` are closed by `RC-resource-budget`).

## Defects found and resolution status

The table below summarizes the seven root causes found by the suite against commit `7bb6d36` and their current status after integrating the fixes in [`proposed-fixes/`](proposed-fixes):

| root cause | slugs | pre-fix cases | post-fix cases | resolution status |
|---|---|---|---|---|
| **The reader had no diagnostics channel.** (`RC-parser-silent`) | 14 slugs: the nine `silent-*`, `lenient-malformed-input`, `expr-truncation`, `dc-clobbers-waveform`, `dc-value-ignored-with-waveform`, `title-line-parsed-as-card` | 172 | **37** (`silent-ignore-model-param` 31, `silent-wrong-model-level` 4, `silent-ignore-directive` 1, `silent-misparse` 1; 135 closed by Phases 1–2) | **Strict reader, expressions, includes, temperature, multipliers, waveforms & `.ic`/`uic` merged.** Remaining unimplemented model parameters emit structured `warning[W_*]` diagnostics and unsupported levels/directives are refused loudly. |
| **Features that are absent** are refused loudly (the honest failure). | `unsupported-syntax-loud`, `unsupported-device-loud` | 45 | **18** (`unsupported-syntax-loud` closed; 18 `unsupported-device-loud` remain for Phase 3) | **`.param` expressions, `.subckt PARAMS`, `.func`, `.include`/`.lib`, `.dc` resistor/`temp`/nested sweeps closed.** Remaining 18 cases are device cards E F H B S W K T O Q J Z U and R/C/BJT/JFET models. |
| **Frozen-pivot unrolled LU.** (`RC-lu-pivot`) | `unrolled-lu-tran-divergence`, `linear-static-pivot-accuracy`, `stale-lu-after-faer-fallback` | 19 | **0** | **Fixed.** Added symmetric minimum-degree ordering, diagonal-preferring threshold Markowitz pivoting (`0.1`), transient pivot refresh (`refresh_for_transient`), factor-growth stability check, and automatic fallback to `DensePartialPivotLu` / `FaerSupernodal`. |
| **Newton box, limits and criteria.** (`RC-newton-clamp`, `RC-newton-limit`) | `clamp-rail`, `false-convergence-pnjlim`, `no-op-homotopy-tran`, `dc-sweep-no-recovery`, `abstol-absolute-residual`, `r-nonpositive-clamp`, `dc-static-homotopy`, `exp-argument-clamp`, `newton-iteration-cap`, `param-floor-clamp` | 54 | **0** | **Fixed.** Removed rail-box clamping, added `pnjlim`/`fetlim` step-delta convergence check, relative KCL residual scaling, smooth diode high-bias linearization (`40 * N * Vt`), adaptive Gmin/source-stepping homotopy, transient sub-stepping, and removed artificial `PHI`/`L` parameter floors. |
| **Time and sweep grid.** (`RC-time-grid`) | `tran-degenerate-last-step`, `dc-sweep-grid-drift`, `step-retry-grid-desync`, `fixed-step-accuracy` | 38 | **0** | **Fixed.** Integer-counted `.tran` and `.dc` grids, divided-difference LTE adaptive step control, waveform breakpoint landing (`PULSE`, `SIN`, `EXP`, `PWL`) with L-stable `BE -> BE -> BDF-2 -> Trapezoidal` restarts, and quantized `hmax / 2^k` `FactorCache`. |
| **No resource budgets.** (`RC-resource-budget`) | `resource-budget`, `nonfinite-result-accepted` | 17 (+ 24 mutant aborts) | **0** | **Fixed.** Added `ResourceLimits` (`--max-elements`, `--max-nodes`, `--max-subckt-depth`, `--max-sweep-points`, `--max-tran-steps`, `--max-newton-iters`), NUL/non-ASCII token rejection, and non-finite value/solution rejection. |
| **Performance on larger systems.** (`RC-faer-slow`) | `faer-nonlinear-slow`, `symbolic-lu-blowup` | 2 judged + 18 slower than ngspice | **1** static judged + 9 slower than ngspice | **Fixed linear/mesh scaling (`FaerLuCache` symbolic-once LU + op-budget routing).** All 2-D/3-D linear meshes and LC ladders are now faster than ngspice-KLU; `check_mem_scaling.py` passes with 3.8–9.3 KB/unknown. |

All additional defects uncovered by `checks/` (`check_card_order_sweep.py`, `check_twin_blocks.py`, `check_mem_scaling.py`, `check_adjoint_fd.py` 15-row truncation, `check_backend_equiv.py` Metal body-effect/EKV mismatch and silent CLI flag fallback, `check_codegen_kernels.py` empty CUDA PTX kernel, and `run_batch_dc_op` solver cloning) have also been fixed and verified (exit 0; see [`checks/README.md`](checks/README.md)).

What the suite found about its own first version, not RustSpice defects (all corrected here): the published ratios of the legacy benchmark came from a 1 ms-quantised `rusage` time taken as solver time, a cold subprocess compared with a warm in-process call, hard-coded zero errors, single runs multiplied by 128 or 400, a transient compared at t = 0 only, and an ngspice labelled KLU that was running SPARSE 1.3 (`.options klu` selects KLU). `docs/benchmark_report.md` is regenerated by `rustspice bench-ngspice`.

## Limits of this suite

* **One reference simulator, one version.** Everything is judged against ngspice-47 (SPARSE 1.3 by default, KLU on request) on one host. ngspice has its own errors (as-is accuracy 1e-4..1e-5 V; crashes on deep `.subckt` nesting, empty files and unknown subcircuits, tagged `ngspice_crash`), which is why accuracy is judged against a tightly converged run and an independent oracle, never against ngspice at default tolerance.
* **The oracles are narrow.** `dc_linear` (R, V, I, G), `dc_nonlinear` (adds diodes), `tran_lti` (linear R, C, L, V, I, G, at most 400 unknowns) and the closed-form `ladder` cover 209 of the 951 cases of the reference run (6 more hit the oracle's wall-clock budget, 5 are outside its specs). MOS accuracy, hierarchy, waveform and temperature cases rest on tight ngspice alone. ngspice's effective eps0 is 3e-6 (relative) above CODATA-2018, so four MOS decks (`acc_dc_mos_uo_tox_transconductance`, `cap_dev_mos_tox_only`, `cap_dev_mos_uo_tox`, `cap_dev_mos_u0`) would still sit 1e-9..5e-8 A away from ngspice for a RustSpice that used the CODATA value: they can only close if it adopts the same constant, and their tolerance is deliberately not loosened.
* **A slug names a mechanism; the suite only sees the symptom.** A tagged case is `known_gap_confirmed` while the expectation is still violated (by a crash only for the two resource slugs), not while the named mechanism is still the cause. The report's known-gaps table shows the observed outcome per slug; a fix for one mechanism can leave a deck failing through another one.
* **The linear oracles bound the error of the largest components.** The forward-error bound of a dense solve is relative to the largest unknown, so a component many decades below it (the far end of a 1e-6..1e15 resistor chain) can carry a large relative error and still be accepted; such a deck is a reference for the large components only.
* **The mutation check is not a proof.** `--teeth` scales one of three elements by 5 %; 455 passing cases see a mutation, none was blunt, but 40 of those see it only through a source, 13 cases have no mutable element, and 68 compare nothing numerically.
* **Timing is one host, one core.** Timed cases run both tools pinned to one P-core of a shared 24-core machine, from tmpfs, end to end as subprocesses. RustSpice's multi-threaded paths are only measured by the batch benchmark of `bench-ngspice` and by the unpinned re-run of the slower cases (thread use was not recorded). Ratios are valid within one report, not across hosts or sessions (two passes of the large-scale chunks at different host loads agreed within 25 % for 74 of 79 cases, but the one SRAM case that the faer tier cannot handle moved from 36 s (3 of 5 repetitions succeeded) to no result within 120 s with the load). Both contenders are launched through the same guarded runner (own process group, watchdog), which adds a small fixed overhead to each (not measured separately): for decks that run in a few milliseconds the ratios are compressed towards 1. Memory is `VmHWM` polled from `/proc`; a child that ends between two polls can read low (`rss_lower_bound` in the JSON).
* **Scale.** Every case that ran stayed under its RSS cap (peaks: 2.0 GiB in the static run, 2.4 GiB at large scale, 3.0 GiB in the mutant runs); three large-scale cases with estimates of 5.3-8.8 GiB were `skipped_budget`. `--scale large` needs the memory estimate printed at start-up and is chunked per family in the reference run. The largest decks are synthetic (ladders of 1e6 resistors, 1e5 parallel diodes, parse-stress decks) and the meshes reach about 6e4 unknowns (2-D, large scale) and 3e3 (3-D): no real 1e5-device design and no extracted parasitic netlist is in the suite.
* **Features RustSpice does not have** (AC, noise, `.meas`, `.step`, BJT, JFET, the controlled sources E F H and behavioural sources) are recorded as gaps (refused or ignored), not measured for accuracy: there is nothing to compare.
* **GPU and WebAssembly backends** are exercised only by `checks/untested_cli_surface`, on one RTX 4090; Metal and WGSL kernels are compared structurally only. The batched DC kernel is compared with its CPU build, the CPU solver and a KCL check by `rustspice-solver`'s `batch_dc` test, on the same single GPU.
* **The verification is statistical, not exhaustive.** A second team re-verified the 330 decks tagged at that time (283 confirmed as stated, 34 re-tagged, 9 ambiguous and 2 non-defects turned into `info`, 2 deck bugs fixed); the engine review found 61 issues (fixed here, except where noted in `SUITE_SPEC.md` 15).
* **`proposed-fixes/` is unreviewed.** The patches were developed independently against the same base; none got a human code review.
