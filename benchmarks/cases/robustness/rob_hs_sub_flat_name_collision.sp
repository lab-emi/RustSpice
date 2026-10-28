top-level node named x1.m collides with the flattened internal node m of instance X1
* bench.id: rob_hs_sub_flat_name_collision
* bench.group: robustness
* bench.title: top-level node named x1.m collides with the flattened internal node m of instance X1
* bench.probes: v(1), v(x1.m), i(v1), i(v2)
* bench.expect: match
* bench.tags: subckt, node_name, degenerate, lenient_ok
* bench.est_rss_mib: 64
* bench.note: both tools flatten internal nodes as <instance>.<node>, so the user node x1.m IS the internal node: i(v1) = -4 mA, i(v2) = +3 mA
.subckt s a b
R1 a m 1k
R2 m b 1k
.ends
V1 1 0 5
X1 1 0 s
V2 x1.m 0 1
.op
.end
