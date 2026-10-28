Series RLC step response, Q = 5
* bench.id: acc_tr_rlc_q5
* bench.group: accuracy
* bench.title: Series RLC step, underdamped (Q = 5), 50 steps per period
* bench.probes: v(out), i(v1)
* bench.tol.norm: 1e-2
* bench.oracle: tran_lti
* bench.series.tstep: 16n 8n 4n 2n 1n 0.5n
* bench.tags: linear, tran, rlc, series
* bench.est_rss_mib: 16
* bench.note: Series: the nominal member (4 ns) carries the verdict; the coarser 16 ns and 8 ns members are info and are not a defect (RustSpice 6.9e-2/1.9e-2 vs ngspice as-is 4.8e-2/1.3e-2, i.e. 1.4-1.5x, the same trapezoid class; oracle agrees with the reference to 1e-4).
V1 in 0 PULSE(0 1 0 1p 1p 1 2)
R1 in a 6.32456
L1 a out 1u
C1 out 0 1n
.tran 4n 2u
.end
