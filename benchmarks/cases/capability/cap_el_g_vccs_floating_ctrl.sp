VCCS controlled by a floating voltage (control nodes not at ground)
* bench.id: cap_el_g_vccs_floating_ctrl
* bench.group: capability
* bench.title: VCCS controlled by a floating voltage (control nodes not at ground)
* bench.probes: v(o), v(c1), v(c2)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: controlled, op, vccs
* bench.note: v(c1)=0.5, v(c2)=-0.5, control voltage 1 V so v(o) = 1 V.
V1 c1 c2 1
R0 c2 0 1k
R1 c1 0 1k
G1 0 o c1 c2 1m
R2 o 0 1k
.op
.end
