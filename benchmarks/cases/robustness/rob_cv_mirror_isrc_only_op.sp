NMOS current mirror with current sources only (no voltage source anywhere)
* bench.id: rob_cv_mirror_isrc_only_op
* bench.group: robustness
* bench.title: NMOS current mirror with current sources only (no voltage source anywhere)
* bench.probes: v(d1), v(d2)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, mirror, current_driven, op
* bench.note: Regression anchor: no V source means the clamp degenerates to [-5,5 V]; nodes stay at ~1.1 V so RustSpice converges (matches ngspice-tight to 2.6e-11 V).
Iref 0 d1 100u
M1 d1 d1 0 0 nm w=10u l=1u
M2 d2 d1 0 0 nm w=10u l=1u
Iout 0 d2 100u
Rp d2 0 10meg
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.op
.end
