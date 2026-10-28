Ground referenced from inside a subcircuit
* bench.id: cap_lang_sub_ground_inside
* bench.group: capability
* bench.title: node 0 inside a .subckt body is the global ground, not a local node
* bench.probes: v(out)
* bench.tags: linear, op, subckt, scoping
.subckt LOAD in
R1 in 0 1k
.ends LOAD
V1 in 0 DC 6
R1 in out 2k
X1 out LOAD
.op
.end
