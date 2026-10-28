Ideal inductor directly across an ideal voltage source, .op
* bench.id: rob_cv_sing_l_across_v_op
* bench.group: robustness
* bench.title: Ideal inductor directly across an ideal voltage source, .op
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: ill_posed, vl_loop, ngspice_error_ok, caught_panic_noise, op
* bench.note: ngspice-47 warns "singular matrix: check node l1#branch" then returns i(l1)=0.05 A (equivalent to a 100 ohm path); RustSpice refuses ("Singular Jacobian matrix encountered at column/row 0") after printing a caught faer panic message (caught_panic_noise, cosmetic). A DC short across an ideal source has no solution (5 V across 0 ohm); refusal is correct.
V1 1 0 5
L1 1 0 1m
.op
.end
