Diode driven by 5 V through 0.5 mohm (about 7.9 kA)
* bench.id: rob_cv_diode_fwd_kiloamp_op
* bench.group: robustness
* bench.title: Diode driven by 5 V through 0.5 mohm (about 7.9 kA)
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, large_current, op
* bench.note: ngspice-47 v(2)=1.0658104 V, i=7868.38 A; numpy oracle agrees. 401 A (R=10 mOhm) still converges in RustSpice, so the cliff is an absolute-residual effect. Calibration evidence: an independent dense f64 Newton (gmin->0) matches ngspice-tight to 7e-14 V, and its own KCL residual at the root is 1.8e-11 A - the same rounding noise RustSpice reports (1.455e-11 = 2^-36 = one ulp of a term of 6.5e4..1.3e5 A; the largest KCL term is gd*v = 3.2e5 A), so an f64 solver meets 1e-12 A here only by rounding luck. It is not a clean threshold: R=1 mOhm (3.95 kA), 0.7 mOhm and 0.3 mOhm (13 kA) happen to converge, only the rounding luck of that one residual differs. Reference verified: tight ngspice agrees with an independent 50-digit mpmath solve (Vt = 0.025864917007155805, gmin = 1e-12) to 6.4e-14 (v(2)) and 1.7e-14 (i(v1)); ngspice as-is is 1.0e-5 and 2.8e-6 off.
V1 1 0 5
R1 1 2 0.5m
D1 2 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
