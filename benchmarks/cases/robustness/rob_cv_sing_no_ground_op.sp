Circuit without any ground node (V1 between nodes 1 and 2, R across it)
* bench.id: rob_cv_sing_no_ground_op
* bench.group: robustness
* bench.title: Circuit without any ground node (V1 between nodes 1 and 2, R across it)
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: ill_posed, no_ground, ngspice_error_ok, caught_panic_noise, op
* bench.note: ngspice-47 warns "singular matrix: check node 2" and returns v(1)=5.005 V, v(2)=0.005 V (gmin-defined); RustSpice refuses ("Singular Jacobian matrix encountered at column/row 0") after printing a caught faer panic message (caught_panic_noise, cosmetic). The common-mode potential is undetermined (only v(1)-v(2)=5 V is meaningful), so a regularised answer with a warning would be as acceptable; the case pins today's refusal.
V1 1 2 5
R1 1 2 1k
.op
.end
