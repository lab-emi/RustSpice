VCCS multiplier m=3
* bench.id: cap_el_g_m_multiplier
* bench.group: capability
* bench.title: VCCS multiplier m=3
* bench.probes: v(o1)
* bench.expect: match
* bench.known_gap: silent-ignore-multiplier
* bench.gap.note: m=3 is ignored: v(o1)=4 V instead of 12 V
* bench.tags: controlled, op, vccs, multiplier, silent_risk
* bench.note: Exact: i = m*gm*v(in) = 3*2m*2 V = 12 mA into o1, v(o1) = 12 V (tight ngspice 12 V). Only m is dropped: RustSpice with G=6m also gives 12 V.
V1 in 0 2
G1 0 o1 in 0 2m m=3
R1 o1 0 1k
.op
.end
