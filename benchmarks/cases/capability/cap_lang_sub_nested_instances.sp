Subcircuit that instantiates another subcircuit
* bench.id: cap_lang_sub_nested_instances
* bench.group: capability
* bench.title: OUTER instantiates INNER twice; INNER is a plain 2-resistor divider leg
* bench.probes: v(out), v(x9.mid)
* bench.tags: linear, op, subckt, hierarchy
.subckt INNER a b
R1 a b 1k
R2 b 0 1k
.ends INNER
.subckt OUTER in out
X1 in mid INNER
X2 mid out INNER
.ends OUTER
V1 in 0 DC 8
X9 in out OUTER
R9 out 0 10k
.op
.end
