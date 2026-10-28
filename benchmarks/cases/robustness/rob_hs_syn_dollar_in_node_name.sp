dollar sign inside a node name
* bench.id: rob_hs_syn_dollar_in_node_name
* bench.group: robustness
* bench.title: '$' inside a node name is not a comment
* bench.probes: v(1), v(n$1), i(v1)
* bench.expect: match
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: RustSpice cuts every line at the first '$', even inside a token, so 'R1 1 n$1 1k' becomes 'R1 1 n' and the deck is refused ('Resistor requires R<name> <n+> <n-> <value>'); ngspice-47 treats '$' as a comment only after whitespace or a comma (verified: '1k $ x' and '1k,$ x' are comments, '1k$ x' is a netlist error) and solves the divider (v(n$1) = 2.5 V)
* bench.tags: hostile, syntax, parser, node_name
* bench.est_rss_mib: 64
* bench.note: node names such as net$1 are common in netlists exported by layout and schematic tools The refusal is incidental to the resistor card: 'I1 0 n$1 1m' is truncated silently to 'I1 0 n' (the source value is lost, exit 0, v(n) = 0 V instead of 1 V) when no later card refuses.
V1 1 0 5
R1 1 n$1 1k
R2 n$1 0 1k
.op
.end
