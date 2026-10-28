Three series PMOS pull-up (gates grounded, gamma=0.5) into a 100k load
* bench.id: rob_cv_pmos_stack3_body_op
* bench.group: robustness
* bench.title: Three series PMOS pull-up (gates grounded, gamma=0.5) into a 100k load
* bench.probes: v(a), v(b), v(out)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, stack, body_effect, op
* bench.note: Regression anchor; RustSpice matches ngspice-tight to 1.5e-10 V.
Vdd vdd 0 3.3
M1 a 0 vdd vdd pm w=20u l=1u
M2 b 0 a vdd pm w=20u l=1u
M3 out 0 b vdd pm w=20u l=1u
R1 out 0 100k
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02 gamma=0.5 phi=0.7)
.op
.end
