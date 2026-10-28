RC with a step larger than the time constant
* bench.id: acc_tr_rc_coarse_step
* bench.group: accuracy
* bench.title: RC (tau = 1 us) with tstep = 3 us (3 tau): trapezoid alternates sign on a stiff step, backward-Euler decays monotonically
* bench.probes: v(out)
* bench.tol.norm: 5e-2
* bench.oracle: tran_lti
* bench.tags: linear, tran, coarse_step
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 6u 1n 1n 18u 36u)
R1 in out 1k
C1 out 0 1n
.tran 3u 72u
.end
