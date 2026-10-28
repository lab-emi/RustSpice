Two inductors in parallel fed through a resistor (inductor-only loop, DC current split undetermined)
* bench.id: rob_cv_sing_l_loop_op
* bench.group: robustness
* bench.title: Two inductors in parallel fed through a resistor (inductor-only loop, DC current split undetermined)
* bench.probes: v(1), v(2)
* bench.expect: loud_error
* bench.tags: ill_posed, inductor_loop, ngspice_error_ok, caught_panic_noise, op
* bench.note: ngspice-47 warns "singular matrix: check node l2#branch" and splits the 5 mA arbitrarily (i(l1)=3.33 mA, i(l2)=1.67 mA); RustSpice refuses ("Singular Jacobian matrix encountered at column/row 0") after printing a caught faer panic message (caught_panic_noise, cosmetic). The split of DC current between ideal parallel inductors is undetermined (consistent but non-unique, v(1) and v(2) are well defined), so a regularised answer with a warning would be as acceptable as the refusal; the case pins today's refusal.
V1 1 0 5
R1 1 2 1k
L1 2 0 1m
L2 2 0 2m
.op
.end
