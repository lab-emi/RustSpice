RC with a step larger than the time constant
* bench.id: acc_tr_rc_coarse_step
* bench.group: accuracy
* bench.title: RC (tau = 1 us) with tstep = 3 us (3 tau): trapezoid alternates sign on a stiff step, backward-Euler decays monotonically
* bench.probes: v(out)
* bench.tol.norm: 5e-2
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: Trapezoid takes the 1 V step at tstep = 3 tau as 0.6 V then alternates around the exact decay (0.35 of swing); ngspice (breakpoint + LTE) is within 1.7e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, coarse_step
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 6u 1n 1n 18u 36u)
R1 in out 1k
C1 out 0 1n
.tran 3u 72u
.end
