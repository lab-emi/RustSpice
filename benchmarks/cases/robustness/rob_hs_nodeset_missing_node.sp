nodeset on a node that does not exist
* bench.id: rob_hs_nodeset_missing_node
* bench.group: robustness
* bench.title: .nodeset on a node that does not exist
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, directive, lenient_ok
* bench.est_rss_mib: 64
* bench.note: ngspice warns 'Nodeset on non-existent node, ignored'
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.nodeset v(nosuch)=3
.op
.end
