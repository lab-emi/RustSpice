Inductor is a DC short, capacitor is a DC open
* bench.id: cap_el_lc_dc_short_open_op
* bench.group: capability
* bench.title: Inductor is a DC short, capacitor is a DC open
* bench.probes: v(b), v(c), v(d), i(l1), i(v1)
* bench.expect: match
* bench.tags: passive, op, reactive_dc
* bench.note: L1 shorts b to c (v(b)=v(c)=4 V, i(l1)=2 mA); C1 blocks DC so v(d) = 6*1k/4k = 1.5 V and i(v1) = -(2+1.5) mA = -3.5 mA.
V1 a 0 6
R1 a b 1k
L1 b c 1m
R2 c 0 2k
C1 c d 1u
R3 d 0 1k
R4 a d 3k
.op
.end
