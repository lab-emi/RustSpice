PMOS 1:1 current mirror, IREF=50 uA, output held at 2.5 V
* bench.id: cap_dev_ckt_pmos_mirror_op
* bench.group: capability
* bench.title: PMOS 1:1 current mirror, IREF=50 uA, output held at 2.5 V
* bench.probes: i(vo), v(g)
* bench.expect: match
* bench.tags: textbook, op, pmos, mirror
VDD vdd 0 5
IREF g 0 50u
M1 g g vdd vdd PM W=20u L=2u
M2 out g vdd vdd PM W=20u L=2u
VO out 0 2.5
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
.op
.end
