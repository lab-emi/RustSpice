dc V1 0 5 0
* bench.id: rob_hs_dc_step_zero
* bench.group: robustness
* bench.title: .dc V1 0 5 0
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, dc, args
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 0 5 0
.end
