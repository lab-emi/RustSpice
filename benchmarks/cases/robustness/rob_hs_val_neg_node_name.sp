node names that look like negative numbers (-1, -2)
* bench.id: rob_hs_val_neg_node_name
* bench.group: robustness
* bench.title: node names that look like negative numbers (-1, -2)
* bench.probes: v(-1), v(-2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: hostile, value, node_name
* bench.est_rss_mib: 64
V1 -1 0 5
R1 -1 -2 1k
R2 -2 0 1k
.op
.end
