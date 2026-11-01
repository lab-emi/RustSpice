# Development status

Updated **2026-11-01**. This page describes the source published with the
RustEDA development update. Use the superproject’s exact gitlinks and the
repository lockfile when reproducing a run.

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

RustSpice is not timed in Ibex RTL-to-GDS. It has no Ibex speedup claim. BJT,
JFET, MESFET and MOS levels above 1 remain unsupported. The benchmark command
writes its report outside published documentation unless an output is explicit.

[Whole-flow evidence and measurement scope](https://lab-emi.github.io/RustEDA/benchmarks/) ·
[RustEDA engine status](https://lab-emi.github.io/RustEDA/engine-status/)
