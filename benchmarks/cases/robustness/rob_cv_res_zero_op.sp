Resistor of 0 ohm in series with 1k (a short)
* bench.id: rob_cv_res_zero_op
* bench.group: robustness
* bench.title: Resistor of 0 ohm in series with 1k (a short)
* bench.probes: v(2), i(v1)
* bench.expect: either
* bench.tags: linear, zero_resistor, ill_posed, op
* bench.note: Ambiguous by design - nobody can be judged: both tools give v(2)=5 V but the branch current is wrong in both (exact answer i(v1)=-5 mA by Ohm's law with R1=0). ngspice-47 reports -4.8828125 mA (-5/1024, as-is and tight identical, no warning; the cause is not known, ngspice is a black box here) and RustSpice 640 A (R1 clamped to 1e-18 ohm, slug r-nonpositive-clamp). Because the reference is itself 2.3 % off the exact value this deck cannot be a known_gap against ngspice (SUITE_SPEC 13.4.1); informational only. A loud refusal of R=0 would be the cleanest behaviour.
V1 1 0 5
R1 1 2 0
R2 2 0 1k
.op
.end
