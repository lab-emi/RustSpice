Two-stage CMOS OTA in unity-gain feedback (open-loop gain ~1e4), current-source tail and load
* bench.id: rob_cv_ota_follower_op
* bench.group: robustness
* bench.title: Two-stage CMOS OTA in unity-gain feedback (open-loop gain ~1e4), current-source tail and load
* bench.probes: v(out), v(d1), v(d2), v(tail)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, feedback, high_gain, op
* bench.note: Reference verified: ngspice-47 gives v(out)=1.4998793 V (offset 0.12 mV) and solves this .op directly (no gmin or source stepping in its log; only the .dc sibling needs dynamic gmin stepping). An independent dense f64 Newton (textbook Level-1 equations, source stepping, analytic Jacobian) reaches the same root with KCL residual 6e-20 A; with gmin->0 it equals ngspice run with gmin=1e-18 to 1e-10 V, and 80 converged random starts in [-5,5] V found only this one root (unique). Mechanism check: +-300 V clamp-widening dummies rescue Vin=0.9, 1.2 and 2.4 V but not 1.5, 1.8 or 2.1 V; at Vin=1.5 V dummy +-X V pairs converge for X = 50, 1e4 and 1e9 and fail for X = 8.3, 10, 20, 100, 300, 1e3 and 1e6 (chaotic), and lowering the open-loop gain (lambda 0.02 -> 0.2) makes plain RustSpice converge, so the failure is Newton non-convergence of the high-gain loop (MOS evaluated without fetlim/damping), not the clamp (widening it is not a fix). Forcing the faer LU or abstol=1e-9 does not help (private patched build, reproduced); second-look flip (private patched build, not reproduced here): a 0.3 V or 1 V global node-step limit makes this .op converge; slug kept at the registered dc-static-homotopy (dc-newton-no-limiting is not registered).
Vdd vdd 0 3.3
Vin inp 0 1.5
It tail 0 20u
M1 d1 out tail 0 nm w=20u l=1u
M2 d2 inp tail 0 nm w=20u l=1u
M3 d1 d1 vdd vdd pm w=20u l=1u
M4 d2 d1 vdd vdd pm w=20u l=1u
M5 out d2 vdd vdd pm w=40u l=1u
Ib out 0 40u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.op
.end
