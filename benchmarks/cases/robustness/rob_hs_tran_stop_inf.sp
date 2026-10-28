tran 1n inf
* bench.id: rob_hs_tran_stop_inf
* bench.group: robustness
* bench.title: .tran 1n inf
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, tran, args, nonfinite
* bench.est_rss_mib: 64
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1n inf
.end
