Villard voltage doubler (5 V sine, two 10 uF capacitors, 10k load)
* bench.id: rob_cv_voltage_doubler_tran
* bench.group: robustness
* bench.title: Villard voltage doubler (5 V sine, two 10 uF capacitors, 10k load)
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, doubler, tran
* bench.note: Reference verified: independent scipy Radau integration of the two capacitor voltages (f64, rtol 1e-10, max_step 2 us) agrees with the converged ngspice run to 4.6e-6 of the 8.5 V swing (as-is 1.2e-3). The 0.335 ms failure moves when the netlist is perturbed by a harmless 1 pF across each diode or a 1 Mohm leak at the floating node b (RustSpice then fails at 1.489 ms, the instant ngspice's v(out) first exceeds the clamp), i.e. it depends on matrix values rather than on the physics; a Python re-implementation of the documented Newton loop with partial pivoting never fails at 0.335 ms, and with several frozen static pivot orders it fails at 0.331-0.335 ms (not exactly reproduced). Flips (private patched build, reproduced): neither the faer LU nor clamp widening (+-300 V dummies) completes the deck alone, together they complete at tstep 5u and 0.5u (norm error 5.5e-4 vs tol 5e-3), abstol=1e-9 is not needed; so the 0.335 ms failure is the frozen static-pivot LU (linear-static-pivot-accuracy, the closest registered slug; static-pivot-tran-divergence is not registered) and the clamp-rail failure at 1.489 ms waits behind it. Slug clamp-rail is the necessary part.
V1 a 0 SIN(0 5 1k)
C1 a b 10u
D1 0 b dm
D2 b out dm
C2 out 0 10u
R1 out 0 10k
.model dm D(is=1e-14 n=1)
.tran 5u 10m
.end
