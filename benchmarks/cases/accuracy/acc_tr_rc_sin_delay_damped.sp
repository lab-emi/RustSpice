RC driven by a delayed damped sine
* bench.id: acc_tr_rc_sin_delay_damped
* bench.group: accuracy
* bench.title: RC low-pass (tau = 1 us) driven by SIN(0.2 1 100k 5u 50k): 5 us delay, 50 k/s exponential damping, DC offset 0.2 V, 100 steps per period
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, sin
* bench.est_rss_mib: 16
V1 in 0 SIN(0.2 1 100k 5u 50k)
R1 in out 1k
C1 out 0 1n
.tran 100n 40u
.end
