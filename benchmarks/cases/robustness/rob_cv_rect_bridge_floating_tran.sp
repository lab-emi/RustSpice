Diode-bridge rectifier fed by a floating 10 V source (no ground reference on the source)
* bench.id: rob_cv_rect_bridge_floating_tran
* bench.group: robustness
* bench.title: Diode-bridge rectifier fed by a floating 10 V source (no ground reference on the source)
* bench.probes: v(p)
* bench.expect: match
* bench.tags: nonlinear, rectifier, floating_source, tran
* bench.note: Reference verified: independent scipy Radau integration (state = C1 voltage, the two floating bridge nodes solved by a 1-D root find per RHS call, f64, rtol 1e-10) agrees with the converged ngspice run to 1.4e-5 of the 8.45 V swing (as-is 3.6e-3). Mechanism check (Python re-implementation of the documented Newton loop and of the unrolled LU's compile-time pivot selection): with the frozen pivot order [3,2,0,1] the failure time reproduces to 17 digits (3.2500122070312554e-4 s); with partial pivoting there is no failure at 0.325 ms and the run dies at 0.4447 ms where the held output (8.405 V) first exceeds the transient clamp (8.402 V); adding 1 Mohm common-mode resistors from both source terminals to ground moves the real failure to the same 0.4447 ms. Slug clamp-rail is the necessary part (the failure at 0.4447 ms); the early failure belongs to linear-static-pivot-accuracy, the closest registered slug (static-pivot-tran-divergence is not registered). Flips (private patched build, reproduced): clamp widening alone and faer LU alone both fail; widening (+-300 V dummies) with the faer LU completes at tstep 5u and 0.5u (norm error 9.3e-4 vs tol 5e-3), abstol=1e-9 is not needed (widening with abstol alone still fails).
Vs a b SIN(0 10 1k)
D1 a p dm
D2 b p dm
D3 0 a dm
D4 0 b dm
C1 p 0 100u
R1 p 0 1k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
