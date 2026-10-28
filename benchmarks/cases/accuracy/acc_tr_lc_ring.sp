Lossless series LC driven by a step: 20 periods of ringing
* bench.id: acc_tr_lc_ring
* bench.group: accuracy
* bench.title: Lossless LC (f0 = 5.03 MHz) step response over 20 periods, 50 steps per period
* bench.probes: v(out)
* bench.tol.norm: 1.2e-1
* bench.oracle: tran_lti
* bench.series.tstep: 8n 4n 2n 1n 0.5n
* bench.tags: linear, tran, lc, ringing, series
* bench.est_rss_mib: 16
* bench.note: Loosened to 1.2e-1: at 50 steps per period the trapezoid phase error accumulates to 8 % of the swing over 20 periods in BOTH tools (RustSpice 8.25e-2, ngspice default 8.24e-2, oracle agrees); the series shows the 2nd-order convergence of both. A regression that adds numerical damping or drift would exceed 1.2e-1. Series: the nominal member (4 ns) carries the verdict; the coarser 8 ns member is reported info by the engine and is NOT a RustSpice defect (RustSpice 0.320, ngspice as-is 0.322, oracle agrees: both tools run the same trapezoid phase error).
V1 in 0 PULSE(0 1 0 1p 1p 1 2)
L1 in out 1u
C1 out 0 1n
.tran 4n 4u
.end
