RC driven by a slow sine
* bench.id: acc_tr_rc_sin_lo
* bench.group: accuracy
* bench.title: RC low-pass (tau = 1 us) driven by a 100 kHz sine, 100 steps per period (10 steps per tau)
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, sin
* bench.est_rss_mib: 16
V1 in 0 SIN(0 1 100k)
R1 in out 1k
C1 out 0 1n
.tran 100n 30u
.end
