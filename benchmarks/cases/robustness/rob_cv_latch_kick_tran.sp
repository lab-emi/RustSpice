Latch written by a 20 uA / 0.2 ns current kick into q (kick placed early, before roundoff resolves the metastable point)
* bench.id: rob_cv_latch_kick_tran
* bench.group: robustness
* bench.title: Latch written by a 20 uA / 0.2 ns current kick into q (kick placed early, before roundoff resolves the metastable point)
* bench.probes: v(q), v(qb)
* bench.tol.norm: 1e-2
* bench.expect: match
* bench.tags: nonlinear, bistable, tran
* bench.note: Regression anchor. The kick must come early: with the same kick at 1 ns ngspice itself resolves the metastable point by roundoff before the kick (as-is and reference disagree by 90%+), so that variant is not a valid test. RustSpice matches the reference to 3.3e-3, same as ngspice as-is (3.6e-3), so tol.norm is 1e-2 for headroom.
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
C1 q 0 20f
C2 qb 0 20f
Iset 0 q PULSE(0 20u 100p 50p 50p 200p 1)
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.tran 5p 2n
.end
