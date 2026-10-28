Resistor multiplier m=4
* bench.id: cap_el_r_m_multiplier
* bench.group: capability
* bench.title: Resistor multiplier m=4
* bench.probes: i(v1)
* bench.expect: match
* bench.known_gap: silent-ignore-multiplier
* bench.gap.note: m=4 is ignored: i(v1)=-1 mA instead of -4 mA
* bench.tags: passive, op, multiplier, silent_risk
* bench.note: Exact: four 1k resistors in parallel, i = -4 mA.
V1 a 0 1
R1 a 0 1k m=4
.op
.end
