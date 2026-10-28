R card with a non-numeric value
* bench.id: rob_hs_syn_r_text_value
* bench.group: robustness
* bench.title: R card with a non-numeric value
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 xyz
.op
.end
