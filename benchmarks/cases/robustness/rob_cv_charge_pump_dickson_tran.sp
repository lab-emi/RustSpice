Two-stage Dickson charge pump (5 V clock, 100 nF stages, 1 Meg load)
* bench.id: rob_cv_charge_pump_dickson_tran
* bench.group: robustness
* bench.title: Two-stage Dickson charge pump (5 V clock, 100 nF stages, 1 Meg load)
* bench.probes: v(n3)
* bench.expect: match
* bench.known_gap: fixed-step-accuracy
* bench.gap.note: With the Newton voltage clamp and step-retry grid shift fixed, RustSpice completes the 100 us run on the 2001-row grid, leaving a 0.18-of-swing fixed-step error because the 10 ns clock edges are under-resolved by the 50 ns step
* bench.tags: nonlinear, charge_pump, switching, tran
* bench.note: Reference verified: independent scipy Radau integration of the three capacitor voltages (nodes eliminated, f64, rtol 1e-9, breakpoints at the clock edges; initial OP by fsolve) agrees with the converged ngspice run to 1.5e-4 of the 9.3 V swing (as-is 9.6e-3). Mechanism check: a line-by-line Python re-implementation of RustSpice's documented Newton/transient loop fails at 10.007 us (RustSpice 10.009 us; exactly the same instant when the diode limiter is disabled) and runs to completion with the clamp disabled; the 100 nF stages give C/h = 2 S so the later step-cut retries only add residual noise (the reported 1.455e-11 A is exactly 1 ulp of a 1e5 A term). The faer panic message is the caught-panic noise of the JIT-to-faer fallback.
Vdd vdd 0 5
Vclk clk 0 PULSE(0 5 0 10n 10n 5u 10u)
D1 vdd n1 dm
C1 n1 clk 100n
D2 n1 n2 dm
C2 n2 0 100n
D3 n2 n3 dm
C3 n3 clk 100n
R1 n3 0 1meg
.model dm D(is=1e-14 n=1)
.tran 50n 100u
.end
