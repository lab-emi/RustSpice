dollar sign inside a node name
* bench.id: rob_hs_syn_dollar_in_node_name
* bench.group: robustness
* bench.title: '$' inside a node name is not a comment
* bench.probes: v(1), v(n$1), i(v1)
* bench.expect: match
* bench.tags: hostile, syntax, parser, node_name
* bench.est_rss_mib: 64
* bench.note: node names such as net$1 are common in netlists exported by layout and schematic tools The refusal is incidental to the resistor card: 'I1 0 n$1 1m' is truncated silently to 'I1 0 n' (the source value is lost, exit 0, v(n) = 0 V instead of 1 V) when no later card refuses.
V1 1 0 5
R1 1 n$1 1k
R2 n$1 0 1k
.op
.end
