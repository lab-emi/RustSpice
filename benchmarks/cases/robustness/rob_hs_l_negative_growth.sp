negative inductance (-1 uH) with 1 kohm: growth rate exactly 1/tstep, 20 ns
* bench.id: rob_hs_l_negative_growth
* bench.group: robustness
* bench.title: negative inductance (-1 uH) with 1 kohm: growth rate exactly 1/tstep, 20 ns
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, value, l_negative, tran, silent_risk
* bench.est_rss_mib: 64
* bench.note: exact (ODE with the 1 ns ramp): v2 grows as e^(t/1ns), v(2)(20 ns) = (e-1)*e^19 = 3.0668e8 V; ngspice with reltol=1e-9 gives 3.0671e8 (1e-4 relative). Not a RustSpice gap (SUITE_SPEC 13.4.1): at default tolerances both simulators accumulate the local error of the unstable mode over 20 e-foldings. ngspice-47 as-is writes 3.9998e8 V at its own t = 20 ns row (0.30 of the swing off; this is its error, not a resampling artifact as an earlier note said), adaptive RustSpice (LTE control at reltol 1e-3, trapezoid) 0.21 off; a 5e-3 global error over 20 e-foldings needs a per-step error near 1e-5, far below any default SPICE tolerance. Kept as information: the former RustSpice result (-1e25 V with exit 0, the backward-Euler singularity of lambda*h = 1 at the fixed step) is what this deck guarded against.
V1 1 0 PULSE(0 1 0 1n 1n 1 2)
R1 1 2 1k
L1 2 0 -1u
.tran 1n 20n
.end
