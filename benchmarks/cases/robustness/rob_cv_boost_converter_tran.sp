Boost converter 5 V -> ~12 V: inductor, NMOS switch (3 us on / 2 us off), diode, 10 uF / 100 ohm
* bench.id: rob_cv_boost_converter_tran
* bench.group: robustness
* bench.title: Boost converter 5 V -> ~12 V: inductor, NMOS switch (3 us on / 2 us off), diode, 10 uF / 100 ohm
* bench.probes: v(out)
* bench.expect: match
* bench.tags: nonlinear, converter, switching, tran
* bench.note: Reference verified: an independent scipy Radau integration of the (iL, vC) ODE (switch node eliminated by a 1-D root find, f64, rtol 1e-9, breakpoints at the gate edges) agrees with the converged ngspice run to 1.3e-5 of the 7.56 V swing (ngspice as-is 1.9e-5), peak 11.8 V; the default tol.norm is kept. RustSpice fails within 49 us at any tstep from 20n to 2n (earlier for finer steps). Second-look verification (private patched build, reproduced): the clamp is necessary but not sufficient: clamp widening alone, faer LU alone and abstol=1e-9 alone all fail, widening (+-300 V dummies) with the faer LU completes the deck at its 20n step (norm error 6.0e-4 vs tol 5e-3), widening with abstol=1e-9 completes 20n, 5n and 2n. Slug clamp-rail is the necessary part (first of the composite clamp-rail + linear-static-pivot-accuracy / abstol-absolute-residual); the registry text "independent of clamp widening" does not hold.
V1 in 0 5
L1 in sw 10u
M1 sw g 0 0 nm w=2000u l=1u
Vg g 0 PULSE(0 5 0 1n 1n 3u 5u)
D1 sw out dm
C1 out 0 10u
R1 out 0 100
.model dm D(is=1e-14 n=1)
.model nm nmos(level=1 vto=1 kp=200u lambda=0.01)
.tran 20n 100u
.end
