Half-wave rectifier, 10 V peak 1 kHz sine, 100 uF / 1k
* bench.id: rob_cv_rect_halfwave_10v_tran
* bench.group: robustness
* bench.title: Half-wave rectifier, 10 V peak 1 kHz sine, 100 uF / 1k
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, rectifier, tran
* bench.note: Reference verified: independent scipy Radau integration of the single capacitor ODE (f64, rtol 1e-10) agrees with the converged ngspice run to 3.6e-6 of the 9.2 V swing (as-is 1.9e-3). With a +40 V or a +-100 V dummy source (clamp widened) RustSpice completes and is 1.7e-3 from Radau (below the 5e-3 default), so accuracy is fine once the clamp is out of the way and the mechanism is the clamp alone.
V1 in 0 SIN(0 10 1k)
D1 in out dm
C1 out 0 100u
R1 out 0 1k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
