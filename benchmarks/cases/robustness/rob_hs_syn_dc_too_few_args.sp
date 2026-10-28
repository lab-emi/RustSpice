dc with three arguments
* bench.id: rob_hs_syn_dc_too_few_args
* bench.group: robustness
* bench.title: .dc with three arguments
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, dc
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 0 5
.end
