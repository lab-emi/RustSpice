Internal subcircuit nodes are local
* bench.id: cap_lang_sub_internal_node_scope
* bench.group: capability
* bench.title: two instances each own an internal node 'mid'; a top-level node also named 'mid' stays separate
* bench.probes: v(mid), v(x1.mid), v(x2.mid)
* bench.tags: linear, op, subckt, scoping
.subckt HALF in
R1 in mid 1k
R2 mid 0 1k
.ends HALF
V1 a 0 DC 8
V2 b 0 DC 4
X1 a HALF
X2 b HALF
R9 a mid 1k
R8 mid 0 3k
.op
.end
