Series RLC step response, Q = 0.5
* bench.id: acc_tr_rlc_q0p5
* bench.group: accuracy
* bench.title: Series RLC step, critically damped (Q = 0.5), 50 steps per period
* bench.probes: v(out), i(v1)
* bench.tol.norm: 1e-2
* bench.oracle: tran_lti
* bench.tags: linear, tran, rlc
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 0 1p 1p 1 2)
R1 in a 63.2456
L1 a out 1u
C1 out 0 1n
.tran 4n 1u
.end
