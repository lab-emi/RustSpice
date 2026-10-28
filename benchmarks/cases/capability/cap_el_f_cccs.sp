Current-controlled current source F (gain 10 on the current of V1)
* bench.id: cap_el_f_cccs
* bench.group: capability
* bench.title: Current-controlled current source F (gain 10 on the current of V1)
* bench.probes: v(out)
* bench.expect: match
* bench.tags: controlled, op, cccs, unsupported
* bench.note: Gap closed 2026-10-28: RustSpice now models the F card (CCCS); the evidence below records the reference and the behaviour before the fix. Hand value: i(V1) = -1 mA (SPICE sign, current into the + terminal), F pushes 10*(-1 mA) = -10 mA into out, so v(out) = -10 V; ngspice-47 returns exactly -10. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 1
R1 in 0 1k
F1 0 out V1 10
R2 out 0 1k
.op
.end
