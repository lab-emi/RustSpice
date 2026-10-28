dc V1 2 2 1 (single point)
* bench.id: rob_hs_dc_start_equals_stop
* bench.group: robustness
* bench.title: .dc V1 2 2 1 (single point)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: dc, args, lenient_ok
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 2 2 1
.end
