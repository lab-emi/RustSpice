Nonlinear E source with POLY(1) (v = 0 + 1*vin + 0.5*vin^2)
* bench.id: cap_el_e_poly_nonlinear
* bench.group: capability
* bench.title: Nonlinear E source with POLY(1) (v = 0 + 1*vin + 0.5*vin^2)
* bench.probes: v(out)
* bench.expect: match
* bench.tags: controlled, op, vcvs, poly, nonlinear, unsupported
* bench.note: Gap closed 2026-10-28: RustSpice now models E ... POLY(1) (Newton-linearised polynomial); the evidence below records the reference and the behaviour before the fix. Hand value: v = 0 + 1*vin + 0.5*vin^2 = 2 + 2 = 4 V; ngspice-47 returns exactly 4. Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 2
E1 out 0 poly(1) in 0 0 1 0.5
R1 out 0 1k
.op
.end
