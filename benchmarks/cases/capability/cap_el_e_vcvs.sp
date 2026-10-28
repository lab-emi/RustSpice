Linear voltage-controlled voltage source E (VCVS, gain 3)
* bench.id: cap_el_e_vcvs
* bench.group: capability
* bench.title: Linear voltage-controlled voltage source E (VCVS, gain 3)
* bench.probes: v(out)
* bench.expect: match
* bench.tags: controlled, op, vcvs, unsupported
* bench.note: Gap closed 2026-10-28: RustSpice now models the E card (VCVS); the evidence below records the reference and the behaviour before the fix. Hand value: v(out) = 3*v(in) = 6 V; ngspice-47 returns exactly 6. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 2
E1 out 0 in 0 3
R1 out 0 1k
.op
.end
