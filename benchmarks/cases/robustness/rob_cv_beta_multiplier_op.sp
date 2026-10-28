Self-biased beta-multiplier reference (two DC solutions: all-off and the operating point)
* bench.id: rob_cv_beta_multiplier_op
* bench.group: robustness
* bench.title: Self-biased beta-multiplier reference (two DC solutions: all-off and the operating point)
* bench.probes: v(n1), v(p1), v(s)
* bench.tol.rel: 1e-5
* bench.expect: match
* bench.tags: nonlinear, mos, multiple_solutions, op
* bench.note: Regression anchor for a circuit with a degenerate all-off solution: both simulators land on the operating point (v(n1)=1.10653 V); RustSpice differs from ngspice-tight by 2.3e-8 V (ngspice as-is 2.5e-9), so tol.rel is 1e-5.
Vdd vdd 0 3.3
M1 n1 n1 0 0 nm w=10u l=2u
M2 p1 n1 s 0 nm w=40u l=2u
Rs s 0 5k
M3 p1 p1 vdd vdd pm w=20u l=2u
M4 n1 p1 vdd vdd pm w=20u l=2u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.op
.end
