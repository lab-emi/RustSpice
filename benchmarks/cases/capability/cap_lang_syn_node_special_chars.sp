Special characters in node names
* bench.id: cap_lang_syn_node_special_chars
* bench.group: capability
* bench.title: node names with + - [ ] / : (n+, n-, bus[1], a/b, x:y) as used by schematic netlists
* bench.probes: v(n+), v(n-), v(bus[1]), v(a/b), v(x:y)
* bench.tags: linear, op, syntax, node_names
V1 n+ 0 DC 12
R1 n+ n- 1k
R2 n- bus[1] 1k
R3 bus[1] a/b 1k
R4 a/b x:y 1k
R5 x:y 0 1k
.op
.end
