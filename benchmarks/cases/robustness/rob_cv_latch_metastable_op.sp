Cross-coupled CMOS latch, default OP (unstable symmetric solution)
* bench.id: rob_cv_latch_metastable_op
* bench.group: robustness
* bench.title: Cross-coupled CMOS latch, default OP (unstable symmetric solution)
* bench.probes: v(q), v(qb)
* bench.expect: match
* bench.tags: nonlinear, bistable, op
* bench.note: Regression anchor: from a zero start both simulators land on the symmetric metastable point (1.5975393 V); RustSpice agrees to 4e-14 V.
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.op
.end
