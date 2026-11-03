# Development status

Updated **2026-11-02**. This page describes the source published with the
RustEDA development update. Use the superproject’s exact gitlinks and the
repository lockfile when reproducing a run.

## Ibex host campaign · 2026-11-02

The RustEDA host campaign reran Ibex with two real SRAMs on IHP SG13G2 / EZ130
on a 4-vCPU Intel Xeon cloud host (15.7 GiB RAM, no swap). Its SPICE
head-to-head ran `rustspice bench-suite` with RustSpice 0.1.0 at commit
[`cc2426f`](https://github.com/lab-emi/RustSpice/commit/cc2426f3e6cbb1ad05843b22a71632f118f62ddc)
(release build, the source described below) against ngspice-45 from nixpkgs,
timing both of ngspice's direct solvers, SPARSE (its default) and KLU.

**Scope: linear RC interconnect only.** IHP SG13G2 transistors use PSP103
models, which RustSpice does not implement, so no transistor-level Ibex
simulation was run. The decks are built from the native nominal SPEF of the
final RustEDA Ibex layout. Each deck drives real nets with a Thevenin ramp,
keeps the extracted resistors (resistances floored at a small minimum) and
positive capacitors with coupling capacitance grounded, and adds a fixed load
capacitance at each sink pin:

| Deck | Contents | Unknowns |
| --- | --- | ---: |
| `ibex_net_00` … `ibex_net_07` | One extracted net each, the eight largest; 3–10 sinks; 2 probes (driver and first sink) | 548–649 |
| `ibex_bulk_10` | 10 extracted nets in one deck; 8 probes, on the first 4 nets | 5,843 |
| `ibex_bulk_40` | 40 extracted nets in one deck; 8 probes, on the first 4 nets | 20,023 |

Accuracy is judged at the probed nodes only, against a tightly converged ngspice
transient reference (`reltol=1e-6 abstol=1e-15 vntol=1e-9`, `tmax = 2 ps`); a
case passes when the maximum error / swing of every probe is at most
2 × 10⁻³ (set in each deck; the suite default is 5 × 10⁻³). The reference's
2 ps maximum step is coarser than the single-net decks' output grid: 3,021
reference rows against 6,001 RustSpice rows. Default-tolerance ngspice is
scored against the same reference. Timing uses the correctness run
as warm-up, then seven interleaved repetitions in rotating order; every
repetition completed and no series is `noisy` (stddev / median > 0.2). Speedup
is median ngspice / median RustSpice against the faster ngspice solver for that
metric.

| | 8 single nets | 10 nets, 5,843 unknowns | 40 nets, 20,023 unknowns |
| --- | ---: | ---: | ---: |
| Accuracy verdict | `pass` × 8 | `pass` | `pass` |
| Error / swing, RustSpice | 0.90–2.12 × 10⁻⁴ | 7.90 × 10⁻⁴ | 7.68 × 10⁻⁴ |
| Error / swing, default-tolerance ngspice | 0.66–1.59 × 10⁻⁴ | 1.62 × 10⁻³ | 1.69 × 10⁻³ |
| Reference resampling floor (`interp_floor`) | 1.96–4.22 × 10⁻⁴ | 2.93 × 10⁻⁴ | 4.63 × 10⁻⁴ |
| Median end to end, ngspice SPARSE | 534.33–618.43 ms | 1,107.13 ms | 4,072.28 ms |
| Median end to end, ngspice KLU | 630.54–727.85 ms | 1,203.12 ms | 4,097.51 ms |
| Median end to end, RustSpice | 136.28–161.45 ms | 196.78 ms | 704.24 ms |
| Median solve, ngspice SPARSE | 522.67–604.48 ms | 1,040.85 ms | 3,821.68 ms |
| Median solve, ngspice KLU | 616.70–714.24 ms | 1,137.76 ms | 3,885.93 ms |
| Median solve, RustSpice | 106.52–126.09 ms | 134.53 ms | 488.71 ms |
| End-to-end speedup vs the faster ngspice solver | 3.68–4.18× | 5.63× | 5.78× |
| Solve-only speedup vs the faster ngspice solver | 4.60–5.14× | 7.74× | 7.82× |
| Median peak RSS, ngspice SPARSE | 15,404–15,708 KiB | 30,724 KiB | 71,608 KiB |
| Median peak RSS, ngspice KLU | 15,536–15,784 KiB | 31,800 KiB | 75,680 KiB |
| Median peak RSS, RustSpice | 33,728–38,416 KiB | 46,232 KiB | 144,152 KiB |

**The suite verdict is `PASS`: 10 of 10 cases pass, and RustSpice was 3.68–5.78×
faster end to end than the faster ngspice solver**, which was SPARSE in every
case for both metrics. RustSpice's worst probe used 39 % of the tolerance. On
the single nets both errors are below the suite's estimate of the reference's
own linear-resampling error (`interp_floor`), so the comparison does not
resolve which simulator is closer; on the two multi-net decks both errors
exceed that estimate and RustSpice is closer. RustSpice used more memory than
ngspice in every case.

End to end is the subprocess wall time: start-up, parse, setup, solve and
rawfile write. Solve is ngspice's `Total analysis time` and RustSpice's
`--stats` `analysis_us`, which excludes RustSpice's parse and compile (the
Cranelift JIT / symbolic LU build): 2.73–3.31 ms on the single nets, 31.25 ms
and 96.40 ms on the multi-net decks. Peak RSS is `VmHWM` polled while each child
runs. Process start-up medians over 15 repetitions are 6.77 ms for ngspice and
2.47 ms for RustSpice. All values come from one session on one shared host
(load average 0.4 at start, 1.0 at end); ratios are comparable only within it.

This host's ngspice is ngspice-45; the retained differential suite below and
the [benchmark method](benchmark_report.md) cite ngspice-47. RustSpice is not
part of the timed RTL-to-GDS flow, so it contributes no end-to-end flow time.

[SPICE head-to-head](https://lab-emi.github.io/RustEDA/benchmarks/#rustspice-vs-ngspice-45) ·
[Host campaign method and deviations](https://lab-emi.github.io/RustEDA/benchmarks/#host-campaign-method-and-deviations) ·
[Benchmark method](benchmark_report.md)

## Adaptive simulation and bounded device kernels

The simulator source includes adaptive transient integration and breakpoint
handling, strict SPICE parsing with expressions/includes and explicit resource
limits, extended diode/level-1 MOS and controlled-source coverage, and adjoint
parameter sensitivities. Batched FP64 DC has a deterministic CPU reference and
an optional CUDA kernel with explicit fallback for unsupported/nonconverged cases.

The retained ngspice-47 accuracy suite reports 857 passing, 16 documented gaps,
77 informational and one unverified case out of 951, with no failing cases;
400 random decks and 200 hostile inputs are recorded separately. These are
accuracy/robustness results from the existing campaign, not new performance
measurements made for this publication.

RustSpice is not timed in Ibex RTL-to-GDS and has no flow-level speedup claim;
its only Ibex measurement is the linear RC head-to-head above. BJT, JFET,
MESFET and MOS levels above 1 remain unsupported, and so do the PSP103
transistor models of IHP SG13G2. The benchmark command writes its report
outside published documentation unless an output is explicit.

[Whole-flow evidence and measurement scope](https://lab-emi.github.io/RustEDA/benchmarks/) ·
[RustEDA engine status](https://lab-emi.github.io/RustEDA/engine-status/)
