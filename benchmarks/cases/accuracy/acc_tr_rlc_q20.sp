Series RLC step response, Q = 20
* bench.id: acc_tr_rlc_q20
* bench.group: accuracy
* bench.title: Series RLC step, high-Q (Q = 20), 50 steps per period
* bench.probes: v(out), i(v1)
* bench.tol.norm: 2e-2
* bench.oracle: tran_lti
* bench.tags: linear, tran, rlc
* bench.est_rss_mib: 16
* bench.note: Tolerance 2e-2: at 50 steps per period RustSpice (1.03e-2) and ngspice default (1.01e-2) share the same trapezoid phase error over 25 periods.
V1 in 0 PULSE(0 1 0 1p 1p 1 2)
R1 in a 1.58114
L1 a out 1u
C1 out 0 1n
.tran 4n 5u
.end
