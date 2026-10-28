Long-time RC pulse train (10,000 steps)
* bench.id: acc_tr_long_rc_pulse
* bench.group: accuracy
* bench.title: RC (tau = 20 steps) driven by a 50 % pulse train whose edges are one step wide, 50 periods = 10,000 steps
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, long_run
* bench.est_rss_mib: 32
V1 in 0 PULSE(0 1 5u 1u 1u 94u 200u)
R1 in out 1k
C1 out 0 20n
.tran 1u 10m
.end
