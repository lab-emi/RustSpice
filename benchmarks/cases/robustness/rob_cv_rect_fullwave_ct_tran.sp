Center-tapped full-wave rectifier (two anti-phase 10 V sources), 100 uF / 1k
* bench.id: rob_cv_rect_fullwave_ct_tran
* bench.group: robustness
* bench.title: Center-tapped full-wave rectifier (two anti-phase 10 V sources), 100 uF / 1k
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, rectifier, tran
* bench.note: Reference verified: independent scipy Radau integration of the single capacitor ODE (f64, rtol 1e-10) agrees with the converged ngspice run to 3.6e-6 of the 9.2 V swing (as-is 1.9e-3). With a +40 V or +-100 V dummy source RustSpice completes and is 1.7e-3 from Radau (below the 5e-3 default): the clamp alone is the cause.
V1 a 0 SIN(0 10 1k)
V2 b 0 SIN(0 -10 1k)
D1 a out dm
D2 b out dm
C1 out 0 100u
R1 out 0 1k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
