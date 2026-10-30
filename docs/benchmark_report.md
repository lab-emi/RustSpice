# Benchmark method: RustSpice vs. ngspice-47

RustEDA publishes one benchmark: a matched Ibex RTL-to-GDS comparison with LibreLane on IHP SG13G2 / EZ130. See the [RustEDA benchmarks](https://lab-emi.github.io/RustEDA/benchmarks/). RustSpice publishes no performance comparison; this page describes how `rustspice bench-ngspice` measures one, so you can run it on your own host.

## What the harness measures

| Family | Circuit and analysis |
| --- | --- |
| b1 | Resistor ladder, DC operating point (legacy ladder and a flat ladder with O(1) V at every node) |
| b2 | RC transmission line, transient, plus a `tstep` ladder for time versus accuracy |
| b3 | Nonlinear diode cascade, DC sweep |
| b4 | CMOS inverter chain, transient |
| b5 | Sensitivity of one output to every resistor: one RustSpice adjoint solve against ngspice `.sens` and finite-difference variants |
| b6 | Batch of supply corners: the RustSpice batch API, loops and a `.dc` sweep against ngspice launches, a `.control` loop and a `.dc` sweep |

Each family runs at several sizes. The report also records process start-up floors and log-log scaling exponents over each size ladder.

## Method

- **Simulators**: ngspice-47 is timed with both of its direct solvers, SPARSE 1.3 (its default) and KLU (`.options klu`); headline ratios use the faster of the two per metric. The RustSpice solver kind is shown per case. ngspice runs only as a black-box executable.
- **Temperature**: 27 C (300.15 K) in both tools.
- **Tolerances**: ngspice runs at its defaults (reltol 1e-3, vntol 1e-6 V, abstol 1e-12 A). RustSpice tolerances are fixed internally (`.options reltol/vntol/abstol` warn and are ignored): Newton reltol 1e-6, vntol 1e-10 V, abstol 1e-12 A; transient LTE reltol 1e-3, vntol 1e-6 V, abstol 1e-12 A, trtol 7. Reference runs: op/dc `.options reltol=1e-9 vntol=1e-12 abstol=1e-15` (SPARSE); tran `.options reltol=1e-6 abstol=1e-15 vntol=1e-9` and tmax = tstep / 20 (SPARSE).
- **Step policy**: RustSpice uses adaptive step control (local truncation error from divided differences, trapezoid with backward-Euler / BDF-2 restarts at waveform breakpoints, maximum step `min(tstep, tmax, (tstop - tstart) / 50)`) and writes one rawfile row per nominal `k * tstep` point. ngspice uses its default adaptive step control with maximum step tstep, and `ngspice -b -r` writes its own internal time points. The comparison interpolates RustSpice, ngspice and the reference linearly onto the nominal `k * tstep` grid, so the ngspice error depends on that grid.
- **Timing protocol**: both contenders are subprocesses writing a rawfile (`rustspice run --raw --stats`, `ngspice -b -r`). One warm-up run (also the correctness run), then interleaved repetitions in rotating order. A contender whose own warm-up takes over 1 s / 5 s is repeated at most 5 / 3 times, and heavy multi-launch workloads at most 3; a cap applies only to that contender. Every table shows the repetitions actually completed, and a contender that failed any repetition never enters a headline ratio. Statistics are min, median and stddev; a series is `noisy` when stddev / median > 0.2. End-to-end time is the child's wall time including start-up. Analysis-only time is ngspice `Total analysis time` (which includes its matrix setup) and RustSpice `analysis_us` (which excludes the symbolic analysis / JIT setup booked as `compile_us`); the like-for-like ratio divides by RustSpice `compile_us + analysis_us`.
- **Accuracy protocol**: full waveform, full sweep and all nodes are compared against a tightly converged ngspice reference (probes whose reference never leaves the floor are `vacuous` and excluded; the transient swing floor is 0.033 V). Where an independent oracle exists (exact ladder recursion, dense Newton, linear-circuit transient solution), it judges RustSpice, ngspice at default tolerances and the reference itself. An unmeasured quantity is reported as `n/a`, never 0.

## Reproduce

Build the CLI and put `ngspice` on `PATH`. The report goes to `.work/benchmark-report.md` by default, outside the published documentation:

```sh
cargo build --release -p rustspice-cli
./target/release/rustspice bench-ngspice --output target/bench-ngspice.md --json target/bench-ngspice.json
# options: --reps N (default 7)  --pin-cpu N  --no-timing  --only b1,b5  --ngspice <path>
```

Absolute numbers are valid only for the host and load recorded in the generated report. Compare ratios within one run, not across runs.

## Caveats

- A shared host is noisy; `noisy` marks series whose spread exceeds 20 % of the median. Re-run on a quiet machine before quoting absolute times.
- Hybrid or frequency-scaling CPUs (performance and efficiency cores, turbo) make short processes bimodal. Series are interleaved and repeated so both sides see the same mix; `--pin-cpu N` pins the sequential children to one core for steadier numbers (parallel workloads are never pinned).
- The like-for-like analysis ratio pairs ngspice `Total analysis time` with RustSpice `compile_us + analysis_us`. Their phase boundaries differ, so it is the closest comparable pair, not an exact phase match. The end-to-end ratio needs no assumption about phase boundaries.
- Both transients are adaptive, with different error controls and output points. Their errors at the same nominal step are not the same quantity, so accuracy is measured against a converged reference and the b2 step ladder shows time versus accuracy.
- The b5 / b6 ngspice multi-launch variants write their decks before timing; the process launches themselves are timed.
