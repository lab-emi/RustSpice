Current-controlled voltage source H (2k transresistance on the current of V1)
* bench.id: cap_el_h_ccvs
* bench.group: capability
* bench.title: Current-controlled voltage source H (2k transresistance on the current of V1)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the H card is refused at parse time (`Unsupported device card H1`, exit 1, no panic) where ngspice computes v(out) = -2 V
* bench.tags: controlled, op, ccvs, unsupported
* bench.note: Hand value: v(out) = 2k * i(V1) = 2k * (-1 mA) = -2 V; ngspice-47 returns exactly -2. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 1
R1 in 0 1k
H1 out 0 V1 2k
R2 out 0 1k
.op
.end
