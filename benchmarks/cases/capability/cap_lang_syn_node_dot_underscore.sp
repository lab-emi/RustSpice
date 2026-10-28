Dots, underscores and digits in node names
* bench.id: cap_lang_syn_node_dot_underscore
* bench.group: capability
* bench.title: node names like n_1, a.b, x1y2 and _lead are legal
* bench.probes: v(n_1), v(a.b), v(x1y2), v(_lead)
* bench.tags: linear, op, syntax, node_names
V1 n_1 0 DC 12
R1 n_1 a.b 1k
R2 a.b x1y2 1k
R3 x1y2 _lead 1k
R4 _lead 0 1k
.op
.end
