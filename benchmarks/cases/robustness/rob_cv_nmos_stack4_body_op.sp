Four series NMOS (all gates at 3.3 V, gamma=0.4, phi=0.65) pulling a 10k load low
* bench.id: rob_cv_nmos_stack4_body_op
* bench.group: robustness
* bench.title: Four series NMOS (all gates at 3.3 V, gamma=0.4, phi=0.65) pulling a 10k load low
* bench.probes: v(out), v(a), v(b), v(c)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, stack, body_effect, op
* bench.note: Regression anchor (body effect on stacked devices). tol.abs 1e-8: RustSpice matches ngspice-tight to 5e-10 V.
Vdd vdd 0 3.3
R1 vdd out 10k
M1 out vdd a 0 nm w=20u l=1u
M2 a vdd b 0 nm w=20u l=1u
M3 b vdd c 0 nm w=20u l=1u
M4 c vdd 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02 gamma=0.4 phi=0.65)
.op
.end
