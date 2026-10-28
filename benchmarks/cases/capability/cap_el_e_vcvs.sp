Linear voltage-controlled voltage source E (VCVS, gain 3)
* bench.id: cap_el_e_vcvs
* bench.group: capability
* bench.title: Linear voltage-controlled voltage source E (VCVS, gain 3)
* bench.probes: v(out)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the E card is refused at parse time (`Unsupported device card E1`, exit 1, no panic) where ngspice computes v(out) = 6 V
* bench.tags: controlled, op, vcvs, unsupported
* bench.note: Hand value: v(out) = 3*v(in) = 6 V; ngspice-47 returns exactly 6. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 2
E1 out 0 in 0 3
R1 out 0 1k
.op
.end
