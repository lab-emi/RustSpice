Behavioural voltage source B (V = 3*v(in))
* bench.id: cap_el_b_voltage_expr
* bench.group: capability
* bench.title: Behavioural voltage source B (V = 3*v(in))
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the B card is refused at parse time (`Unsupported device card B1`, exit 1, no panic) where ngspice computes v(out) = 6 V
* bench.tags: controlled, op, behavioural, unsupported
* bench.note: Hand value: V = 3*v(in) = 3*2 = 6 V; ngspice-47 returns exactly 6. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 2
B1 out 0 V = 3*v(in)
R1 out 0 1k
.op
.end
