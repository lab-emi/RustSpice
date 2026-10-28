node 00 is an ordinary node, not ground
* bench.id: rob_hs_num_node_00_is_not_ground
* bench.group: robustness
* bench.title: node 00 is an ordinary node, not ground
* bench.probes: v(1), v(00), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: node_name, dc, lenient_ok
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 00 1k
R2 00 0 1k
.op
.end
