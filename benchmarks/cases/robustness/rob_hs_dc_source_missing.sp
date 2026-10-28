dc of a source that does not exist
* bench.id: rob_hs_dc_source_missing
* bench.group: robustness
* bench.title: .dc of a source that does not exist
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, dc, args
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc VX 0 5 1
.end
