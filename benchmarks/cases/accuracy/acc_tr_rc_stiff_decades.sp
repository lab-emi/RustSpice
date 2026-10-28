Three RC branches with time constants spanning six decades
* bench.id: acc_tr_rc_stiff_decades
* bench.group: accuracy
* bench.title: RC branches tau = 1 ns / 100 us / 1 ms on one trapezoid source with 20 us edges, tstep 2 us (the fast branch is 2000x stiffer than the step)
* bench.probes: v(f), v(m), v(s)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, stiff
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 0 20u 20u 1.48m 3m)
R1 in f 1k
C1 f 0 1p
R2 in m 1k
C2 m 0 100n
R3 in s 1k
C3 s 0 1u
.tran 2u 4m
.end
