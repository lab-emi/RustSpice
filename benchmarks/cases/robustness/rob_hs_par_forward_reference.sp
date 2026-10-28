param defined after its use
* bench.id: rob_hs_par_forward_reference
* bench.group: robustness
* bench.title: .param defined after its use
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: param, parser
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 {late}
R2 2 0 1k
.param late=1k
.op
.end
