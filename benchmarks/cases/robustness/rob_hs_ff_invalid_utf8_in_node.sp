invalid UTF-8 inside a node name
* bench.id: rob_hs_ff_invalid_utf8_in_node
* bench.group: robustness
* bench.title: invalid UTF-8 inside a node name
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: file, encoding, hostile
* bench.est_rss_mib: 64
* bench.note: ngspice also stops with 'UTF-8 syntax error in input deck'
V1 1 0 5
R1 1 n√( 1k
R2 n√( 0 1k
.op
.end
