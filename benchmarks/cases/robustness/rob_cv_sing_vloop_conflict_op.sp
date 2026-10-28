Two voltage sources (5 V and 3 V) in parallel
* bench.id: rob_cv_sing_vloop_conflict_op
* bench.group: robustness
* bench.title: Two voltage sources (5 V and 3 V) in parallel
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: ill_posed, voltage_loop, ngspice_error_ok, caught_panic_noise, op
* bench.note: ngspice-47: "DC solution failed - singular matrix: check node v1#branch" (names the source); RustSpice: "Singular Jacobian matrix encountered at column/row 0" (no offender named) after printing a caught faer panic message (caught_panic_noise, cosmetic). Both refuse; correct, since 5 V = 3 V has no solution.
V1 1 0 5
V2 1 0 3
R1 1 0 1k
.op
.end
