dc of a current source
* bench.id: rob_hs_dc_source_is_current_source
* bench.group: robustness
* bench.title: .dc of a current source
* bench.probes: v(1)
* bench.expect: match
* bench.tags: dc, args
* bench.est_rss_mib: 64
I1 0 1 1m
R1 1 0 1k
.dc I1 0 3m 1m
.end
