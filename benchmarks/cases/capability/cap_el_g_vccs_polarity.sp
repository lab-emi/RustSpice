VCCS polarity: swapped output nodes, negative gm, swapped control nodes
* bench.id: cap_el_g_vccs_polarity
* bench.group: capability
* bench.title: VCCS polarity: swapped output nodes, negative gm, swapped control nodes
* bench.probes: v(o1), v(o2), v(o3), v(o4)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: controlled, op, vccs, polarity
* bench.note: G n+ n- c+ c- gm pulls gm*v(c+,c-) out of n+ and into n-: v(o1)=-4, v(o2)=+4, v(o3)=-4, v(o4)=-4 V.
V1 in 0 2
G1 o1 0 in 0 2m
R1 o1 0 1k
G2 0 o2 in 0 2m
R2 o2 0 1k
G3 0 o3 in 0 -2m
R3 o3 0 1k
G4 0 o4 0 in 2m
R4 o4 0 1k
.op
.end
