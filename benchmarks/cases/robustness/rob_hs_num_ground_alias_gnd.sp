node name gnd is the ground node
* bench.id: rob_hs_num_ground_alias_gnd
* bench.group: robustness
* bench.title: node name gnd is the ground node
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: node_name, dc, lenient_ok
* bench.est_rss_mib: 64
V1 1 gnd 5
R1 1 2 1k
R2 2 0 1k
.op
.end
