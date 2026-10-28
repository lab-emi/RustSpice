Diode + RC driven by a 5 V pulse with 1 ps edges at tstep 1 ns (swing across the diode per step: 5 V)
* bench.id: rob_cv_tran_diode_rc_edge
* bench.group: robustness
* bench.title: Diode + RC driven by a 5 V pulse with 1 ps edges at tstep 1 ns (swing across the diode per step: 5 V)
* bench.probes: v(a), v(b)
* bench.expect: match
* bench.tags: nonlinear, diode, switching, tran
* bench.ref.tmax: 1p
* bench.note: ngspice-47 solves (breakpoints + adaptive step); an independent scipy Radau integration (state v(b), node a by a 1-D root find, breakpoints at the 1 ps edges, rtol 1e-10) agrees with the converged ngspice run to 4.8e-5 of the 4.36 V swing (as-is 9e-4), so the reference is trustworthy. RustSpice passes for amplitude <= 3.5 V, fails for >= 4 V, and passes at 5 V with tstep <= 0.2 ns or edges >= 3 ns, i.e. when the trapezoid overshoot stays inside the 5 V rails. A Python re-implementation of the documented Newton loop reproduces the failure instant exactly (6.001709e-9 s, residual 29.0) and shows Newton pinned at the clamp boundary; the earlier "150 iterations x 26 mV" hypothesis is not what happens. Two mechanisms act on this deck: the abort at 6.002 ns is the clamp-rail defect (the overshoot lies outside the Newton clamp range), but the wrongness underneath is fixed-step trapezoid ringing at tstep >> RC, so the slug stays fixed-step-accuracy: fixing the clamp alone only turns the abort into a wrong answer (v(b) overshooting to 7.02 V against 4.36 V). Flip tests (only the stated change): tstep 0.5 ns still aborts, 0.2 ns completes with v(b) 0.354 of swing off, 3 ns edges instead of 1 ps complete with v(b) 0.125 off (overshoot to 4.89 V), and the +-100 V dummy rail pair completes with v(b) 0.61 off. Reference rows: bench.ref.tmax 1p (the default tstep/20 = 50 ps leaves rows that straddle the 1 ps edges, so the thinned reference is 0.095 of the v(a) swing off at the 6 ns row 1 ps before the falling corner and the resampling floor exceeds tol.norm; 1 ps rows bring the floor to 3e-10; the tolerance is unchanged).
V1 in 0 PULSE(0 5 1n 1p 1p 5n 10n)
R1 in a 10
D1 a b dm
C1 b 0 10p
R2 b 0 10k
.model dm D(is=1e-14 n=1)
.tran 1n 20n
.end
