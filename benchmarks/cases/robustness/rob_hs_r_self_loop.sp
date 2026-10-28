resistor with both terminals on the same node
* bench.id: rob_hs_r_self_loop
* bench.group: robustness
* bench.title: resistor with both terminals on the same node
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, value, dc, lenient_ok
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R3 2 2 1k
.op
.end
