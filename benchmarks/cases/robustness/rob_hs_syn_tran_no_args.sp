tran without arguments
* bench.id: rob_hs_syn_tran_no_args
* bench.group: robustness
* bench.title: .tran without arguments
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, tran
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.tran
.end
