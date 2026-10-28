CMOS Schmitt trigger driven by a triangle
* bench.id: acc_tr_schmitt_trigger
* bench.group: accuracy
* bench.title: Two-inverter Schmitt trigger (10 k / 40 k feedback, 20 fF loads) driven by a 2 us triangle: switching instants and hysteresis in the time domain
* bench.probes: v(x), v(out)
* bench.tol.norm: 5e-2
* bench.tags: nonlinear, tran, mos, hysteresis, timing_edge
* bench.est_rss_mib: 16
* bench.note: 50 % crossings of v(out) against the converged ngspice (edges at 622.2, 1632.2, 2642.2, 3652.2 and 4662.2 ns): RustSpice -1.2, -3.1, +0.66, +0.97 and +0.97 ns, ngspice as-is -0.54, -0.01, -0.65, -0.23 and -0.33 ns. Refining only tstep, RustSpice's first-edge shift is -1.15, -0.65, -0.27 and -0.10 ns at 2, 1, 0.5 and 0.1 ns (about first order, with Newton retries and extra rows at every step), so the defect is real and fixed-step, but a pointwise max-norm over 100 ps edges is hypersensitive to the switching instant and was not converged by any step tried (down to 0.2 ns): ngspice as-is fails too at 1, 0.5 and 0.2 ns (0.97, 0.95, 0.63), and the nominal-step verdict (ngspice 1.4e-2 passes, RustSpice fails) depends on where the grid points fall relative to the edges. The gap is kept because RustSpice's worst switching instant is off by 3.1 ns against 0.65 ns for ngspice at the nominal step (4.8x); a held-rail window (.tran 5n 1.55u 0.8u, bench.floor 3) is not a RustSpice-specific defect (RustSpice 4.5e-3 of the swing, ngspice as-is 1.1e-2), so no second deck was added for it.
VDD vdd 0 DC 3.0
VIN in 0 PULSE(0 3 0 1u 1u 0.01u 2.02u)
R1 in x 10k
RF out x 40k
MP1 m x vdd vdd PMOD W=20u L=1u
MN1 m x 0 0 NMOD W=10u L=1u
CM m 0 20f
MP2 out m vdd vdd PMOD W=20u L=1u
MN2 out m 0 0 NMOD W=10u L=1u
CO out 0 20f
.model NMOD NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02)
.model PMOD PMOS(LEVEL=1 VTO=-0.7 KP=60u LAMBDA=0.02)
.tran 5n 5u
.end
