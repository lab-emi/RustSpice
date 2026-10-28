> **Status of this file.** This is the working design contract the differential suite (`rustspice bench-suite`, the deck corpus in
> `cases/`) was built to. Sections 1-14 are written as instructions to the authors (builders, calibrators) and are kept as they were
> so that the section numbers cited by the engine's diagnostics, its code comments and the decks' `bench.note` lines
> (`SUITE_SPEC 13.1`, `13.4.2`, ...) stay valid. Section 15 records what changed afterwards (registry, stricter metadata, verdict rules).
> Where this file and `README.md` differ, the README and the code are current. Local scratch paths were removed from the text.

# RustSpice differential benchmark suite — design spec v2 (binding contract for all builders)

Owner goal: "review the benchmark of RustSpice against ngspice completely and add more benchmarks to strongly
verify RustSpice's capability, performance and robustness vs. ngspice. Do not commit."

`$M` = the RustSpice module root. The harness lives in the `rustspice-cli` crate
(`rustspice bench-suite`; the legacy `bench-ngspice` is re-implemented honestly on top of the same engine).
ngspice is ONLY a black-box executable (AGENTS.md Rule 1: never read ngspice source/headers; observable output
and timing only). A missing ngspice yields `unverified`, never a pass. Unverified/unsupported quantities are
`null` + a `*_status` string, never `0` (Rule 4). Every run has explicit budgets (Rule 9).

## 0. What the audit established (why the suite looks like this)

Facts every builder must respect (all verified by independent audit agents against ngspice-47 and the shipped
RustSpice 7bb6d36; evidence was kept in the builders' scratch space, not in this repository):

* ngspice-47 here defaults to `Using SPARSE 1.3`; KLU needs `.options klu`. KLU is 4-16x faster than SPARSE on 2-D
  circuits (and slower on 1-D). The old "ngspice-47 (KLU)" label was wrong. The suite times BOTH.
* ngspice `-b -r f.raw deck` (no `.control` block) prints `Total analysis time (seconds) = x` (us resolution) and
  `Total elapsed time`; process start-up floor ngspice ~3.5-5.5 ms, RustSpice CLI ~0.9-1.5 ms. The old harness used
  the 1 ms-quantised `rusage` elapsed time as "solver time" (inflated 8-14x on small decks), compared a cold
  subprocess to a warm in-process call, hard-coded 0.0 errors (b2, b6), multiplied single runs by 400/128 (b5, b6),
  compared t=0 only in the transient benchmark (b4), and compared 1e-34 V values (b1).
* RustSpice: fixed-step transient (BDF-1 first step + trapezoid, no LTE, no breakpoints) -> 0.3 V error on the
  b2 RC line at the published 10 ps; `.options` is ignored (RustSpice always runs reltol 1e-6/vntol 1e-10/abstol 1e-12);
  fixed 300.15 K; the parser silently ignores unknown dot-cards/params (see capability matrix); Newton clamps node
  voltages to the range of the DC sources (stacked-source/rectifier/boost circuits fail with ConvergenceFailure,
  34/36 diode-fuzz failures); small-IS diodes can return a silently wrong OP; no homotopy for the transient initial
  OP (rings fail, CMOS chains >= 200 stages fail); resource bombs abort the process (`.tran 1p 1` => 8 TB alloc,
  recursive subckt => stack overflow); symbolic LU runs to completion before its op cap is checked (dense/3-D fill
  blows memory; the dim=15 000 cliff makes a 14 400-unknown mesh 45x slower than a 15 600-unknown one); faer-tier
  nonlinear solves are 3-70x slower than ngspice-KLU; non-CPU backends execute on the CPU; the Cranelift JIT only
  fires for <= ~34 unknowns.
* RustSpice strengths to keep as regression anchors: linear DC to <= 1e-11 V vs tightly converged ngspice, diode
  DC to 1e-10 V, MOS L1 incl. body effect, subckt flattening, adjoint gradients (6.7e-14 vs oracle; ngspice `.sens`
  is only 1e-6), faer tier correct on 2-D meshes, 1-D DC and linear transient 3-8x faster, rayon batch
  bit-deterministic.
* Differences against DEFAULT-tolerance ngspice (1e-4..1e-5 V) are ngspice's own error; correctness must be judged
  against (a) tightly converged ngspice (`reltol=1e-9 vntol=1e-12 abstol=1e-15`, and for transients the converged
  reference of §3) and (b) independent closed-form / dense f64 oracles.
* The host is noisy (hybrid i9-13900HX, load 12-60 from unrelated jobs). Timing = interleaved A/B, warm-up, min and
  median of N, optional core pinning, noisy flag, loadavg recorded. Trust ratios within one session only.

## 1. Layout

```
$M/benchmarks/
  README.md                      methodology, metric/status definitions, how to run, how to read the report
  cases/<group>/<id>.sp          self-describing decks; group in capability | accuracy | robustness | performance
$M/crates/rustspice-cli/src/bench/
  mod.rs        suite driver, CLI arg structs, exit codes                      (H1)
  types.rs      shared types + generator/oracle signatures                    (skeleton, frozen)
  meta.rs       `* bench.*` metadata parser, deck expansion (series)          (H1)
  rawfile.rs    ngspice-compatible binary rawfile reader AND writer           (H1)
  ngspice.rs    ngspice runner (watchdog, RSS cap, timing parse, solver)      (H1)
  rustsim.rs    RustSpice subprocess runner                                   (H1)
  compare.rs    metrics, classification, work-precision interpolation         (H1)
  stats.rs      min/median/mean/stddev, noisy flag, log-log fits              (H1)
  report.rs     JSON + Markdown report                                        (H1)
  families.rs   scale-ladder deck generators                                  (H2)
  fuzz.rs       seeded random-circuit fuzzers + deck mutator                  (H2)
  oracle.rs     independent f64 oracles (dense LU, closed forms)              (H2)
  legacy.rs     honest re-implementation of the six `bench-ngspice` benchmarks (H3, after merge)
$M/crates/rustspice-cli/tests/      unit/integration tests (ngspice-dependent ones become `unverified` if absent)
```
`main.rs` keeps the clap definitions. `Run` gains `--raw <path>` (write an ngspice-compatible binary rawfile, one plot per
analysis, in execution order) and `--stats <path>` (JSON: `{parse_us, compile_us, solver_kind, solver_compile_us?,
analysis_us, write_us, peak_rss_kib, analysis: "op|dc|tran", points, status}`; unavailable numbers are null).

## 2. Deck metadata (comment lines, the identical deck is fed to BOTH simulators)

The deck is plain SPICE with NO `.control` block and ONE analysis card (`.op`, `.dc` or `.tran`; multi-analysis decks
are allowed only where the case is about multi-analysis behaviour). Metadata lines start with `* bench.` (a comment for
both simulators), may appear anywhere after the title line, one key per line:

```
* bench.id: tran_rlc_underdamped        unique [a-z0-9_]+, equals the file stem
* bench.group: accuracy                 capability|accuracy|robustness|performance
* bench.title: Series RLC step response, Q=5
* bench.probes: v(out), i(v1)           comma list, lower case, ngspice naming; absent => all v() vectors found in BOTH outputs
* bench.tol.rel: 1e-6                   op/dc: pass iff |a-b| <= tol.abs + tol.rel*|ref| for every probe and sweep point
* bench.tol.abs: 1e-9
* bench.tol.norm: 5e-3                  tran: max_t|a-b| / max(swing(ref), floor) <= tol.norm
* bench.floor: 1e-3                     swing floor in probe units (default 1e-6)
* bench.expect: match|loud_error|either match = RustSpice must reproduce the reference within tolerance (default);
                                        loud_error = RustSpice must REFUSE (non-zero exit, no panic, message on stderr);
                                        either = informational
* bench.known_gap: <slug>               RustSpice is KNOWN to violate `expect` today (slug from the registry in
                                        benchmarks/README.md; e.g. clamp-rail, silent-ignore-option, fixed-step-ringing)
* bench.gap.note: free text             one line explaining the observed violation (what RustSpice does today)
* bench.ng.options: reltol=1e-9 ...     extra `.options` for the ngspice AS-IS run (timing/default-accuracy run) only
* bench.ng.solver: sparse|klu|both      default: both for group performance, sparse otherwise
* bench.ref: tight|none|<options>       ngspice REFERENCE run: `tight` = .options reltol=1e-9 vntol=1e-12 abstol=1e-15
                                        (op/dc default); tran default = `reltol=1e-6 abstol=1e-15 vntol=1e-9` plus
                                        tmax=tstep/20 (override with bench.ref.tmax); `none` = compare directly to as-is
* bench.ref.tmax: 0.25p                 tmax of the reference `.tran` (absolute time)
* bench.oracle: <spec>                  independent closed form or dense-LU oracle from oracle.rs (adds err-vs-truth)
* bench.series.tstep: 10p 5p 2p 1p      expands ONE deck into a work-precision series: one case per value (id suffix
                                        `__s<k>`), `.tran` tstep replaced; ngspice runs with tmax = tstep (matching
                                        RustSpice's fixed grid) AND the report builds "time at equal error" tables
* bench.size: 5000                      problem size (unknowns/stages) for scaling fits
* bench.tags: nonlinear, bistable, silent_risk
* bench.timing: yes|no                  default yes for group performance and for series, otherwise no
* bench.est_rss_mib: 300                RustSpice peak-RSS estimate (Rule 9); cases above --max-rss-mib are skipped_budget
* bench.note: free text
```

## 3. Execution protocol per case

1. **ngspice as-is run** (per solver in `bench.ng.solver`): `ngspice -b -r <tmp>/ng.raw <deck'>` with `<deck'>` = deck
   plus `.options klu` and/or `bench.ng.options`; child under watchdog (`--case-timeout`, default 60 s) and RSS cap
   (poll `/proc/<pid>/statm`, kill above `--max-rss-mib`). Parse the rawfile (binary real; use the LAST plot of the analysis
   kind; multi-plot allowed) and `Total analysis time (seconds)`/`Total elapsed time`; verify `Using KLU` / `Using SPARSE`
   in the log. ngspice failed = non-zero exit, no rawfile, or log matches `Error`, `singular matrix`, `no convergence`,
   `Timestep too small`, `Too many iterations`.
2. **ngspice reference run** per `bench.ref` (temp copy of the deck with the options injected; tran gets `tmax`).
   If the reference run fails, fall back to the as-is run and note `ref_status: "fell_back"`.
3. **RustSpice correctness run**: subprocess `current_exe run <deck> --raw <tmp>/rs.raw --stats <tmp>/rs.json` under
   the same watchdog/RSS cap; exit code 101 or SIGABRT/SIGSEGV = `panic`; hang = `timeout`; stderr captured.
4. **Timing** (only when `bench.timing`=yes and steps 1 and 3 succeeded and `--no-timing` absent): `--reps` (default 7,
   min 5) interleaved A/B repetitions after one warm-up each. Per repetition record:
   - `e2e_subprocess`: wall of `ngspice -b -r` vs wall of `rustspice run --raw` (both write the raw file to tmpfs,
     both pay start-up, parse, setup, solve and output) — the headline number;
   - `ng_analysis_us` (`Total analysis time`) and `rs_analysis_us` (from `--stats`) = **solve-only**;
   - `ng_setup_us` = elapsed − analysis (load/parse/setup) vs `rs_parse_us + rs_compile_us`;
   - peak RSS of both children (wait4 `ru_maxrss`; add the `libc` crate — already in Cargo.lock — and refresh offline).
   Report min, median, mean, stddev, n, raw samples, `noisy` (stddev/median > 0.2) and loadavg before/after. Speedup =
   median_ng / median_rs (headline) and min_ng / min_rs; with both ngspice solvers the headline compares against the
   FASTER ngspice solver and prints both. Start-up floors are measured once per suite run with a trivial deck.
5. **Compare + classify** (§4, §5). Artifacts (deck, raw files, stderr, stats) of every non-pass case are kept under
   `--artifacts <dir>` for reproduction.

## 4. Metrics

- op: per probe `abs_err`, `rel_err = abs_err / max(|ref|, floor)`; pass iff `abs_err <= tol.abs + tol.rel*|ref|`.
- dc: same per sweep point after aligning on the sweep variable (tolerant to ±1e-9 relative grid mismatch; a point-count
  mismatch is `fail_accuracy` with note); report max over points and the worst point.
- tran: resample both waveforms onto a uniform grid of the deck's `tstep` over [0,tstop] by linear interpolation
  (record the interpolation-error floor estimated as ref-ngspice vs itself resampled); metric
  `norm_err = max_t |a(t)-b(t)| / max(max(ref)-min(ref), floor)`, plus RMS norm error and (tag `timing_edge`) the maximum
  50 %-crossing time shift. Pass iff `norm_err <= tol.norm`.
- With a reference run report `err(rustspice, ref)` AND `err(ngspice_asis, ref)`; RustSpice is "within ngspice's own
  error" when `err(rs,ref) <= max(err(ng,ref), tol floor)`. With an oracle report `err(rs, oracle)` and `err(ng, oracle)`.
- Never apply a relative metric to values indistinguishable from 0 (|ref| < 1e-30) and never report a pass on a probe
  whose reference is vacuous (all |ref| < floor): such a probe is `vacuous` and excluded with a note.
- Series (work-precision): per point `err_rs`, `err_ng`, `t_rs`, `t_ng`; the report interpolates log-log "time at equal
  error" (solver-only and e2e) for the error levels covered by both curves and states the interpolation range.

## 5. Status vocabulary (exactly these strings)

`pass` | `fail_accuracy` | `fail_robustness` (RustSpice error/hang-free refusal where ngspice solved and expect=match) |
`fail_silent` (expect=loud_error but RustSpice produced numbers) | `known_gap_confirmed` | `gap_closed` | `both_error` |
`ngspice_error` | `panic` | `timeout` | `skipped_budget` | `unverified` | `info`.
Rules: a case with `bench.known_gap` that still violates `expect` is `known_gap_confirmed`; one that no longer violates is
`gap_closed` (doc update needed, listed at the top of the report). A case WITHOUT `bench.known_gap` that violates `expect`
is the matching `fail_*` / `panic` / `timeout`. A panic/abort/timeout/OOM is always a robustness violation, even for
`expect=either`. Run verdict is `FAIL` when any case is fail_*/panic/timeout; `known_gap_confirmed` does not fail the run
but is listed under "Known gaps" (grouped by slug) at the top of the report.

## 6. Families and fuzzers (generated in Rust, deterministic, seed in the report)

`--scale small|medium|large`: small = CI smoke (whole suite < 30 s with `--no-timing`), medium default (< 10 min),
large opt-in with a printed Rule-9 memory estimate (`est_rss_mib`) and refusal above `--max-rss-mib`.

Families (each case embeds full `* bench.*` metadata; ids `fam_<name>_<size>`): 1-D resistor ladder DC (non-vacuous,
all nodes O(1) V, oracle = exact recursion), 2-D resistor mesh DC (natural AND randomly-numbered), RC ladder tran,
2-D RC mesh tran, LC ladder tran, diode chain DC/sweep, diode-resistor mesh sweep, CMOS inverter chain OP+tran, MOSFET
array OP/tran, 6T-SRAM-like OP/tran, star hub (k RC leaves, 100 ohm), parse-stress decks (1k/10k/100k lines), start-up
floor decks. Sizes per scale and the audit-observed cliffs are in `families.rs` doc comments.
Fuzzers (xorshift64*, no external crates): random linear resistor networks (R over 6 decades, connected, dense-LU oracle),
random diode-resistor networks, random RC networks (tran), random CMOS gate networks (DC); and a deck MUTATOR (token-level
mutations of the static corpus: delete/duplicate/truncate lines, swap numbers for NaN/inf/1e999/negative/huge, unbalanced
.subckt/.ends, recursive subckt, garbage bytes) used only to prove RustSpice never panics, hangs, or allocates unboundedly
(each mutant runs under the watchdog; compared with ngspice only for pass/fail class). Fuzz groups report aggregate counts +
the first 5 failing seeds with decks saved under `--artifacts`.

## 7. CLI

```
rustspice bench-suite [--cases <dir>=benchmarks/cases] [--group G]... [--filter SUBSTR] [--family F]...
                      [--scale small|medium|large] [--reps N=7] [--case-timeout S=60] [--max-rss-mib N=4096]
                      [--seed N=1] [--fuzz-count N=25|100|400 by scale] [--json <file>] [--report <md>]
                      [--artifacts <dir>] [--ngspice <path>=ngspice] [--pin-cpu N] [--no-timing] [--no-fuzz] [--list]
rustspice bench-ngspice [--output docs/benchmark_report.md] [other legacy flags unchanged]   (legacy 6, honest methodology)
rustspice run <deck> [--backend B] [--raw <path>] [--stats <path>]
```
Exit code: 0 all good (known gaps allowed), 1 any FAIL, 2 usage/environment error (e.g. no cases found).
A missing `ngspice` => every case `unverified`, exit code 0 only with `--allow-unverified`, else 2 (never a silent pass).

## 8. Reports

JSON `{schema:1, host:{cpu,threads,loadavg_start,loadavg_end,pin_cpu}, ngspice:{version,path,solvers_tested}, rustspice:
{version,git,build_profile}, tolerances:{...}, args, seed, cases:[{id,group,family,title,tags,expect,known_gap,status,
analysis,size,probes:[{name,abs_err,rel_err,worst_at,vacuous}],ref_status,oracle:{...}|null,timing:{...}|null,
timing_status,notes}], series:[...], fuzz:[...], scaling:[{family,points,fit:{ng_sparse,ng_klu,rs}}], summary:{counts}}`.
Markdown sections: (1) verdict + counts by group/status, (2) Known gaps by slug, (3) Capability matrix, (4) Accuracy table
(+ work-precision series), (5) Robustness table + fuzz summary, (6) Performance table (median/min, e2e and solver, parse/
compile split, RSS, speedups vs SPARSE/KLU/fastest, noisy flags), (7) Scaling fits (log-log exponents), (8) Methodology +
caveats (host, load, ngspice solver, tolerances, scope: temperature 27 C, tolerances, step policy), (9) reproduction commands.

## 9. Resource rules for builders (AGENTS.md Rule 9)

Always run builds and anything heavy as
`systemd-run --user --scope --quiet -p MemoryMax=8G -p MemorySwapMax=2G timeout <t> <cmd>` with `CARGO_BUILD_JOBS=2`,
`RUST_TEST_THREADS=2`. `./dev` refuses while another high-memory RustEDA job holds `.work/scale.lock` (an unrelated OpenROAD
job does right now; never kill it) — use `cargo` directly in YOUR OWN scratch copy with a scratch `CARGO_TARGET_DIR`.
Never run two cargo invocations in the same target dir. Never modify the repository outside the files you own
(§10). NEVER `git commit`, `git add`, `git stash`, or change branches anywhere. Estimate peak RAM before any large deck or
fuzz run; cap decks at the sizes in the family tables unless `--scale large`.

## 10. File ownership (disjoint; violating this breaks the merge)

H1 owns bench/{mod,meta,rawfile,ngspice,rustsim,compare,stats,report}.rs, main.rs (Run flags, BenchSuite, legacy wiring),
rustspice-cli/Cargo.toml, tests/bench_engine*.rs. H2 owns bench/{families,fuzz,oracle}.rs and tests/bench_gen*.rs. H3 owns
bench/legacy.rs. Deck authors own ONLY `benchmarks/cases/<their dir>/`. Docs are written by the orchestrator.

## 11. Known-gap slug registry (use ONLY these in `* bench.known_gap:`; propose a new slug in your final report, don't invent silently)

| slug | meaning (what RustSpice does today) | audit ref |
|---|---|---|
| `silent-ignore-directive` | `.include/.lib/.global/.nodeset/.options/.save/.print/.probe` accepted and ignored, undefined `.model`/node refs default silently | a2 F06, a3 F06 |
| `silent-ignore-analysis` | `.ac/.tf/.noise/.sens/.pz/.disto/.step/.meas/.four/.control` replaced by a DC `.op` or dropped | a2 F07 |
| `silent-ignore-model-param` | model/instance params parsed and dropped (diode RS/BV/IBV/TT/CJO/VJ/M/FC/IKF/ISR/NR/EG/XTI, MOS UO->KP, TOX/U0/NSUB/LD/RD/RS, CGBO uses W, no bulk junctions/Meyer caps) | a2 F04/F08 |
| `silent-wrong-model-level` | MOS `LEVEL>=2` (incl. BSIM numbers) silently evaluated as EKV | a2 F03 |
| `silent-ignore-source-waveform` | PWL/EXP/SFFM/AM sources become DC 0 V; PULSE/SIN with too few args become DC; source type found by substring (`Vsin`/`Vpulse` names) | a2 F02, a3 F05 |
| `silent-ignore-temperature` | `.temp`, `.options temp/tnom`, instance `temp/dtemp`, `tc1/tc2` all ignored (fixed 300.15 K) | a2 F05 |
| `silent-ignore-multiplier` | instance `m=` ignored on R/C/L/D/M; positional diode area ignored | a2 F09, a4 F13 |
| `silent-ignore-ic` | `uic`, element `ic=`, `.tran tstart/tmax`, `.ic V(x) = v` (spaced) ignored; `.ic` applied after the OP | a2 F10, a3 F09 |
| `expr-truncation` | `{2*rv}`-style expressions starting with a number truncated; unparsable args dropped and shift positions | a2 F01 |
| `dc-clobbers-waveform` | `.dc` overwrites the swept source waveform and never restores it; nested `.dc` second sweep ignored | a2 F13, a3 F08 |
| `clamp-rail` | Newton clamps node voltages to the DC-source range: stacked/negative rails, current-driven nodes, rectifier/boost/charge-pump fail with ConvergenceFailure | a3 F02, a6 F08 |
| `false-convergence-pnjlim` | Newton declares convergence while the diode limiter is active: small-IS diode OP silently wrong | a3 F01 |
| `stale-lu-after-faer-fallback` | stale JIT LU buffer reused after faer fallback: silent garbage in linear `.dc`/`.tran` | a3 F03 |
| `no-op-homotopy-tran` | transient initial OP has no homotopy and ignores UIC: rings fail, CMOS chains >= 200 stages fail (`.op` >= 400) | a5 F02, a6 F07 |
| `dc-sweep-no-recovery` | DC sweep points have no gmin/source-stepping recovery (hysteresis/Schmitt sweeps fail) | a2 F14 |
| `fixed-step-accuracy` | fixed-step BDF1+trapezoid, no LTE/breakpoints: large error at coarse steps, undamped ringing, lost pulses, tstep-dependent PULSE edges | a3 F04, a1 F3 |
| `abstol-absolute-residual` | absolute 1e-12 KCL residual criterion on mixed units: fails at kA scale and on MOS arrays at default tolerance; `.options` cannot loosen it | a3 F11, a5 F03 |
| `resource-budget` | no work/memory budgets: `.tran 1p 1`/huge sweeps abort or OOM; recursive `.subckt` overflows the stack; non-finite values accepted | a3 F12, a2 F15 |
| `symbolic-lu-blowup` | symbolic LU built to completion before its cap: inductor ladders O(n^3), star hubs, random numbering, dim<=15000 cliff | a5 F04-F07 |
| `faer-nonlinear-slow` | nonlinear systems on the faer tier are 3-70x slower than ngspice-KLU (performance-only; accuracy is fine) | a5 F09 |
| `r-nonpositive-clamp` | `R <= 0` silently clamped to 1e-18 ohm (R=0 gives wrong-sign branch currents) | a3 F10 |
| `linear-static-pivot-accuracy` | static-pivot unrolled LU loses digits on ill-conditioned/badly-numbered linear nets (up to 7e-5 V) | a5 F08, a3 F18 |
| `batch-all-or-nothing` | one failing corner discards the whole batch; AREA override no-op; no temperature in batch | a4 F4-F6 |
| `adjoint-coverage` | the CLI adjoint supports R, diode IS, MOS W/VTO (+ a few); V/I source sensitivities cannot be requested; one unsupported spec aborts; DC only | a4 F8-F9 |

## 12. Oracle specs (`* bench.oracle:`; implemented by H2 in oracle.rs; authors may use exactly these names)

- `dc_linear` — dense f64 LU on the deck itself: R, V, I, G(VCCS) elements (flat netlist, `.param` numbers allowed); `.op` only; probes `v(n)`, `i(vname)`.
- `dc_nonlinear` or `dc_nonlinear(gmin=1e-12)` — dense f64 Newton (damped, 1e-15 convergence) on R,V,I,G,D (Shockley IS/N with
  GMIN across every junction; no RS/BV/CJO) from the deck; `.op` and `.dc` (single source sweep).
- `tran_lti` — dense f64 MNA + L-stable/high-order implicit integrator with Richardson-verified fine fixed step (documented in
  oracle.rs), for linear R, C, L, V, I, G circuits with DC/PULSE/SIN/PWL sources; <= 400 unknowns; `.ic` honoured; the
  oracle's own unit tests validate it against closed forms (RC, RLC, LC).
Outputs also carry `i(v...)` (SPICE sign convention: current into the + terminal flows through the source to -).
`outcome` (JSON per case) = `match|mismatch|loud_error|silent_numbers|panic|timeout|ngspice_only_error|both_error`, independent of
`status`; the capability matrix is drawn from it.

## 13. Addendum (phase 2, written after the first merge) — BINDING

State: the engine (H1), generators/oracles (H2) and 716 static decks (D1-D7) are merged into
`$M` and build. Release binary of the merged tree:
a frozen copy in the integrator's scratch space (builders used their own `CARGO_TARGET_DIR` under `/dev/shm`).
Disk: `/tmp` (tmpfs) was 100 % full from other jobs; `/dev/shm` has ~46 GB. Put ALL scratch (raw files, targets, logs) in
`/dev/shm/<your-name>/`, delete it when done, never fill `/tmp`, never delete files that are not yours.

### 13.1 Policy: how a loud refusal of a feature ngspice supports is recorded
* `expect: loud_error` is ONLY for input where the CORRECT behaviour is to refuse (hostile/invalid input, input ngspice also
  rejects, resource bombs, genuinely out-of-model requests such as a `.ac` sweep result that cannot be produced by a DC tool).
* A feature/syntax that ngspice computes a result for, which RustSpice refuses loudly, is `expect: match` +
  `known_gap: unsupported-syntax-loud` (netlist syntax, `.param` expressions, `.subckt PARAMS`, `.dc temp/R`, non-UTF-8 comments)
  or `unsupported-device-loud` (device cards E F H B S W K T O Q J Z U ... and `.model` cards of R/C/BJT/JFET/MESFET types).
  Reason: when implemented the case must flip to `gap_closed`, not to `fail_silent`.
* A feature RustSpice silently ignores/misreads = the existing `silent-*` slugs. Silent acceptance of malformed input that
  ngspice rejects = `lenient-malformed-input`.

### 13.2 Slug registry additions (in `meta.rs::KNOWN_GAP_SLUGS`, final list = §11 + these)
`unsupported-syntax-loud`, `unsupported-device-loud`, `lenient-malformed-input`, `silent-misparse` (valid syntax read as something
else without error: `G ... POLY(1)` => zero-gain VCCS, value suffix `x` read as meg), `dc-value-ignored-with-waveform` (`DC <v>` attribute
ignored at .op/.dc when a waveform is present; also `dc=3.3`), `tran-newton-diode-commutation` (transient Newton fails on
diode-commutation circuits independent of tstep / clamp widening), `dc-newton-no-limiting` (high-gain feedback OTA .op fails where
ngspice's dynamic gmin stepping solves), `exp-argument-clamp` (diode exponent argument saturated at 80 => confident wrong current),
`ill-posed-dc-silent` (current-source cutsets / cap-only nodes regularised by gmin and returned as 1e6-1e9 V with exit 0),
`title-line-parsed-as-card` (a first line starting with `.` is parsed as a card; ngspice always takes line 1 as title).
Registry total = 24 + 10 = 34 slugs. Slugs RESOLVED as duplicates: `source-dc-attr-ignored` => `dc-value-ignored-with-waveform`;
`no-semantic-validation` and `strict-utf8-input` => `lenient-malformed-input` / `unsupported-syntax-loud`; `step-retry-grid-desync` stays under
`fixed-step-accuracy`; `gmin-ds-leakage` is informational (no deck is tagged, mention in the README only).

### 13.3 Engine semantics the corpus relies on (E1 implements; tags are documented in benchmarks/README.md)
* **Sidecar metadata**: if `<deck>.sp.meta` exists next to a deck it is read as the metadata block (same `* bench.*` syntax, lines
  without the leading `*` also accepted) and the deck file itself is fed verbatim to both tools. Needed for decks that cannot carry
  comments (empty file, whitespace-only, UTF-16 BOM, invalid-UTF-8).
* **Tags with engine semantics**: `ngspice_error_ok` (ngspice may print `singular matrix`/gmin warnings and still write a raw file
  with exit 0; its log patterns must not mark it failed; if it nevertheless exits non-zero or crashes the case is `ngspice_error`
  or `both_error`, never a harness failure), `ngspice_unbounded` (do not run ngspice at all: expected to run away; record
  `ngspice_status: "skipped_unbounded"` and judge RustSpice against `expect` only; the reference for matching is `none`),
  `npoints` (tran: also compare row counts: rows must equal the ngspice row count or floor(tstop/tstep+1e-9)+1 (+1 for a partial last
  step) — fail_accuracy otherwise), `caught_panic_noise` (informational: RustSpice prints a caught faer panic message and exits 1;
  classification stays keyed on exit code 101/abort), `lenient_ok` (informational only).
  E1 must inventory EVERY tag in the corpus (`grep -rhoE 'bench.tags:.*'`) and either give it semantics, document it as
  informational, or flag it.
* **Tran comparison grid**: window = [first ngspice time row, min(tstop, last ngspice row)]; honour `.tran tstart` (ngspice emits rows only
  for t>=tstart, RustSpice from 0); for tstep > tstop and parameterised `.tran` cards derive the grid from the ngspice rows, not from parsing.
* **Includes**: decks with relative `.include/.lib` targets must resolve for both tools: run both children with cwd = the deck's directory
  and keep the injected-options temp deck elsewhere (check empirically that ngspice then still finds the file), or write the temp deck
  next to the original and delete it after the run. Never leave files in the repository.
* **ngspice rc < 0** (SIGSEGV etc.) is retried ONCE; a deterministic crash is `ngspice_error` (the corpus has decks on which
  ngspice-47 itself segfaults: unknown subckt, port-count mismatch, include loops, deep nesting, empty file).
* **Plot selection** is by plot name + analysis kind (ngspice stores plots by type, not card order); multi-analysis decks compare each kind.
* **Reference tolerance**: `RefMode::Options` must default `tmax=tstep/20` for tran like `RefMode::Default` unless `bench.ref.tmax` is set.

### 13.4 Honest-comparison rules (reviewers will check these)
1. A `known_gap_confirmed` is only legitimate if the reference is trustworthy: tight/converged ngspice AND (where an oracle/closed form
   exists) agreement of the reference with it. If ngspice's as-is error is of the same size as RustSpice's the case is NOT a RustSpice gap.
2. Tolerances may not be loosened to make a failing deck pass unless the note proves the loosened level is the reference's own accuracy.
3. Never edit RustSpice or ngspice behaviour; the suite reports defects, it does not fix them.
4. Every deck keeps its `bench.gap.note` one-liner stating what RustSpice does today.

## 14. Addendum (phase 2b, after the second merge) — BINDING

State after the second merge: engine E1 (13.2/13.3 semantics), generators/oracles F1, legacy `bench-ngspice` rewrite H3 and 36 performance
decks D8 are in `$M`. Static corpus run (`--no-timing --no-fuzz --scale small`, 772 cases without the 36 perf decks): pass 445,
known_gap_confirmed 256, info 48, fail_silent 9, fail_robustness 7, fail_accuracy 4, ngspice_error 2, skipped_budget 1 = 20 failing
cases that need metadata (list in each calibrator's brief). Binary for calibrators: a frozen copy of the integrator's build ( Rust files
are NOT edited by calibrators, so it never changes under them).

### 14.1 Registry: four more slugs (meta.rs now has 38; calibrators cannot add slugs, they PROPOSE)
* `tran-degenerate-last-step` — the transient loop (`while t < tstop - 1e-9*tstep`, `t += h`) can end > 1e-9*tstep short of tstop and
  then appends a final step of 1e-20..1e-13 s; the row it writes is garbage for stiff circuits (C/h ~ 1e32 S) and the row count is wrong.
  Repro: RC mesh `.tran 10p 100n` -> 10002 rows, last dt 1.1e-20 s, last value 6.4e3 V for a 0.167 V node. `acc_tr_final_time_sliver`
  (RustSpice writes 10002 rows where `floor(tstop/tstep)+1` = 10001) belongs here; E1's proposal `tran-extra-final-row` is replaced by it.
* `dc-sweep-grid-drift` — `.dc` builds the grid with repeated `cur += step` and a guard `cur <= stop + 1e-12*step`; after N additions the
  rounding exceeds the guard and the LAST POINT IS DROPPED (`.dc V1 0 5 1m` gives 5000 instead of 5001 points; swept values drift ~1e-12 V).
* `tran-newton-mos-switching` — transient Newton fails at a fast (100 ps) MOS edge into small caps, independent of the step; ngspice solves.
* `unrolled-lu-tran-divergence` — transient on a well-conditioned 2-D RC / diode-RC mesh with >= 66 unknowns (UnrolledSymbolic tier):
  exponentially growing garbage, NaN or `TransientFailure` (linear case writes the non-finite values with exit 0); ngspice is within
  1e-4 of an independent solution. Root cause unconfirmed (do not guess a mechanism in the slug); `linear-static-pivot-accuracy` stays for
  the <= 7e-5 V loss of digits on badly numbered nets only.
Final registry = 38 slugs. `singular-matrix-panic` is NOT a slug: a caught panic message plus a clean exit 1 is `caught_panic_noise` (cosmetic).

### 14.2 Calibration protocol (every deck owner follows it; one owner per deck-name prefix; nobody edits Rust, the registry, other owners' decks)
1. Run your decks with the frozen binary, `--no-timing --no-fuzz --scale small --case-timeout 20 --max-rss-mib 2048`, writing JSON/MD/artifacts
   under `/dev/shm/<your-name>/` (never into `$M`). Wrap in `systemd-run --user --scope -q -p MemoryMax=8G -p MemorySwapMax=2G timeout <s> ...`.
2. Triage EVERY non-`pass` case into exactly one of: (a) engine bug (report to the integrator, do not work around it in the deck),
   (b) deck bug (fix it: metadata, probes, tolerance only with evidence per 13.4, deck text), (c) genuine RustSpice defect (tag the registered slug +
   `bench.gap.note`; the expectation stays `match` unless refusing is correct per 13.1), (d) ngspice quirk/own error (tag per 13.3: `expect: either`,
   `ngspice_error_ok`, `ngspice_crash`, `ngspice_unbounded`, or a sidecar), (e) ambiguous: neither side is provably right -> `expect: either`
   with a note that says exactly why nobody can be judged.
3. Verify EVERY `known_gap_confirmed` independently: (i) the reference must be trustworthy — recompute the expected value by hand/python/dense f64
   (closed form, mpmath, numpy, scipy, matrix exponential) or with tight ngspice (`reltol=1e-9 vntol=1e-12 abstol=1e-15`) and show agreement with the
   reference side; (ii) the slug must match the MECHANISM you observed (try a minimal variant of the deck that flips the behaviour); (iii) if RustSpice's
   as-is error is of the size of ngspice's own error the deck is NOT a gap (13.4.1). A gap whose evidence you cannot reproduce is demoted
   (fix the tag or the expectation). Keep one evidence line per slug-group in your report, not per deck.
4. Sample at least 15 % (min 10) of the `pass` decks of your group and check they are not vacuous: probes non-trivial, tolerance not looser than
   the reference's own accuracy, the circuit actually exercises the feature named in the id. Fix or tighten decks that are vacuous.
5. `bench.id` must equal the file stem (`<stem>.sp`) in every deck you own (183 `rob_hs_*` ids lack the prefix). Ids use `[a-z0-9_-]+`.
6. You may ADD decks only inside your own prefix (list of wanted new decks in your brief). Each new deck is validated like the others.
7. Report format (structured): before/after status counts for your group, changed decks, evidence per slug, engine bugs, proposed slugs,
   remaining non-pass cases with a one-line reason each. Final self-check: re-run your group; zero `fail_*`, `unverified`, `ngspice_error`
   unless the reason is a genuine RustSpice defect that has no registered slug (then say so explicitly).

### 14.3 Engine work after the merge (E2; owns `bench/*.rs` and `tests/bench_engine.rs` only)
* Per-child peak RSS must come from `/proc/<pid>/status` `VmHWM` (poll + final sample) or the child's own `--stats`, never from `wait4` `ru_maxrss`
  (it is floored by the harness's own high-water mark: identical values for RustSpice/ngspice-SPARSE/ngspice-KLU in D8's JSON).
* `--teeth` (default off, reported when on): a mutation-sensitivity check for every case with status `pass` and a numeric comparison. RustSpice-only
  re-runs on mutated copies of the deck (scale the first, a middle and the last value-carrying R/C/L/V/I/G/MOS-W element by 1.05; skip decks with
  `.param`/subckt value indirection and non-simple cards) compared against the ORIGINAL ngspice data with the case's own tolerances. A pass whose
  every mutation still passes is `teeth: blunt` (the comparison cannot see the circuit); `teeth: ok|blunt|n/a|skipped` plus the mutated element names go
  into JSON, a summary count into the Markdown report. Never changes `status`; `blunt` cases are listed so deck owners can tighten or retarget them.
* `faer-nonlinear-slow` (and any `PERF_GAP_SLUGS`) is judged on END-TO-END median speedup versus the faster ngspice solver (the user-visible metric),
  `gap_closed` only when speedup >= 1.1 and the series is not noisy; the solve-only ratio goes into the note. Never judge on solve-only.
* The "resampling floor exceeds tol.norm" note must never appear on a pass and must not contradict a fail: only when |err - tol| < floor the case
  is `unverified` with that reason; otherwise the note is dropped or purely informational.
* Fuzz/mutant artifact cap: keep the first 3 artifacts PER DISTINCT FAILURE SIGNATURE (status + first stderr line shape), not 5 per family, so a
  common root cause cannot hide a different defect.
* Timing mode (`--reps >= 5`, scratch cwd with include symlinks, `timing_status` values) must be exercised end to end on the performance decks and
  on one deck with an `.include`; fix whatever breaks.
* Run the fuzz and mutant families (seeds 1 and 2, `--fuzz-count 100`) against the merged engine and triage anything that is not a registered gap.

## 15. Addendum (phase 3/4: independent re-verification, engine hardening; 2026-10-28)

State: every `known_gap_confirmed` deck was re-run by a second team that tried to refute the claim (283 of 330 confirmed as
stated, 34 carried a wrong slug, 9 were ambiguous, 2 were not defects, 2 were deck bugs); the engine was reviewed independently
(61 findings) and hardened; 69 new decks (87 new cases, series members included) and the `checks/` scripts were added.

### 15.1 Registry: final 40 slugs
* Added (a mechanism the second look found under the wrong slug): `nonfinite-result-accepted` (exit 0 with NaN/inf in the output),
  `step-retry-grid-desync` (a Newton retry shifts the output grid off the requested one; its own slug, no longer under
  `fixed-step-accuracy`), `newton-iteration-cap` (150 iterations per Newton call fail on long chains), `param-floor-clamp`
  (MOS `PHI` floored at 0.1 and `L` floored at 1e-12).
* Renamed: `dc-newton-no-limiting` -> `dc-static-homotopy` (the DC solve uses a fixed gmin schedule with no retry).
* Removed: `tran-newton-diode-commutation`, `tran-newton-mos-switching` (every deck under them failed through `clamp-rail` or
  `unrolled-lu-tran-divergence`).
* Definitions widened or corrected: `unrolled-lu-tran-divergence` and `linear-static-pivot-accuracy` share one root cause (the frozen
  x = 0 pivot order of the unrolled symbolic LU; the CraneliftJit tier uses the same factorisation); `r-nonpositive-clamp` also covers
  `R` below the floor; `silent-misparse` is any valid syntax read as something else (not only the `x` suffix); `exp-argument-clamp`
  starts at an exponent argument of 80; `silent-ignore-multiplier` is `m=` on R/C/L/D/M **and G** (a positional diode area cannot be
  tested: ngspice rejects it).
* Registered with no deck tagged in the static corpus: `stale-lu-after-faer-fallback` (not reproduced), `ill-posed-dc-silent` (the
  ill-posed decks became `expect: either`, nothing can be judged), `symbolic-lu-blowup` (generated families only),
  `batch-all-or-nothing` and `adjoint-coverage` (CLI surface, covered by `checks/untested_cli_surface`).

### 15.2 Metadata rules enforced by the parser (violations are diagnostics, the case is `unverified`)
* `bench.known_gap` needs `bench.gap.note` (one line: what RustSpice does today).
* `bench.known_gap` with `bench.expect: either` is rejected, except `resource-budget` (a panic, abort, timeout or OOM violates `either`).
* A performance slug (`faer-nonlinear-slow`) needs `bench.expect: match`.
* A line that only looks like metadata (`Bench.expect: x`, `bench expect: x`) is diagnosed instead of being read as a comment.
* An oracle spec outside section 12 is a note (`oracle: unsupported`); a series whose members are all coarser than the deck's own `.tran` step is rejected.

### 15.3 Verdict rules (compare.rs)
* A panic or timeout is a known gap only for `resource-budget` and `symbolic-lu-blowup`; under any other slug it stays `panic` / `timeout`.
* Non-finite reference data, a one-row reference, or a reference that is not comparable are `unverified`, never a pass or a RustSpice failure.
* A probe whose reference is below the default floor (1e-6, unit-blind) and whose deviation exceeds the tolerance is `unverified` unless the
  deck sets `bench.floor`.
* `expect: either` with a non-crash known gap is `info` ("contradictory metadata"), never `gap_closed`.
* The in-process oracles run under a wall-clock budget `min(10 s, case timeout / 2)`; a deck may raise it with `budget=<s>` in its oracle spec.
  An oracle that exceeds it reports `budget`. A linear oracle is rejected as `ill-conditioned` when its componentwise forward-error bound
  exceeds 1e-8 of the largest component.
* Several analyses of one kind in one deck are paired by span (tran end time, sweep range), not by order (ngspice stores them in reverse).

### 15.4 Harness rules (mod.rs, ngspice.rs, rustsim.rs)
* Exit 2: usage error, nonexistent or empty `--cases`, unusable ngspice (version probe or a start-up self-test of a resistor divider fails),
  a scratch-directory write failure, a RustSpice child that exits 74 (rawfile could not be written). `--strict-unverified` makes any
  `unverified` case exit 2. `--families-only` runs the generated decks without reading `--cases`.
* Children run in their own process groups under a wall-clock timeout and a polled RSS cap; SIGTERM/SIGINT/SIGHUP kill the groups and remove the
  scratch directories; timed repetitions are not guarded by a parent-death signal.
* Timing needs successful samples, not attempts; fewer than 3 samples are reported with `spread_status` (`two_samples`, `single_sample`) and
  `noisy: true`.
