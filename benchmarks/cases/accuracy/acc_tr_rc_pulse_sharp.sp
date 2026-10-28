RC response to a pulse with 1 ps edges
* bench.id: acc_tr_rc_pulse_sharp
* bench.group: accuracy
* bench.title: RC (tau = 10 ns) driven by a PULSE with 1 ps edges on the 1 ns step grid (10 steps per tau)
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, timing_edge
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 10n 1p 1p 100n 200n)
R1 in out 1k
C1 out 0 10p
.tran 1n 400n
.end
