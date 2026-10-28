voltage source with both terminals on the same node
* bench.id: rob_hs_sing_vsource_self_loop
* bench.group: robustness
* bench.title: voltage source with both terminals on the same node
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, singular, dc
* bench.est_rss_mib: 64
V1 1 1 5
R1 1 0 1k
V2 1 0 1
.op
.end
