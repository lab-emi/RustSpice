Latch held in the q=0/qb=3.3 state by .ic (no uic), 20 fF per node
* bench.id: rob_cv_latch_ic_tran
* bench.group: robustness
* bench.title: Latch held in the q=0/qb=3.3 state by .ic (no uic), 20 fF per node
* bench.probes: v(q), v(qb)
* bench.tol.norm: 5e-3
* bench.floor: 1e-3
* bench.expect: match
* bench.tags: nonlinear, bistable, ic, tran
* bench.note: Regression anchor: ngspice holds the .ic state (swing 4e-9 V, hence floor 1e-3 V so that a wrong state would be a ~1600x violation); RustSpice also holds the state for both orientations (q=3.3 and q=0 tested).
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
C1 q 0 20f
C2 qb 0 20f
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.ic v(q)=0 v(qb)=3.3
.tran 20p 2n
.end
