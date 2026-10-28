zero inductance in a transient
* bench.id: rob_hs_l_zero_tran
* bench.group: robustness
* bench.title: zero inductance in a transient
* bench.probes: v(2), v(3)
* bench.expect: match
* bench.tags: hostile, value, l_zero, tran
* bench.est_rss_mib: 64
V1 1 0 PULSE(0 1 0 1n 1n 1 2)
R1 1 2 1k
L1 2 3 0
R2 3 0 1k
.tran 10n 100n
.end
