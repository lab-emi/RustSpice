Capacitor directly across a ramping voltage source (i = C dv/dt)
* bench.id: cap_el_c_across_vsource_ramp
* bench.group: capability
* bench.title: Capacitor directly across a ramping voltage source (i = C dv/dt)
* bench.probes: i(v1)
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: passive, tran, ringing, silent_risk
* bench.note: Exact: i(v1) = -C*dV/dt = -1 nF * 1 V/us = -1 mA during the rising ramp, +1 mA during the falling ramp, 0 elsewhere. Ramp corners sit 2 ns after a grid point (an exact-on-grid corner would make the max-error metric depend on how the discontinuity is sampled); the trapezoid ringing amplitude is (1-2f) of the step, f being the fractional corner position, so f=0.1 gives 80 %. ngspice resolves the corners with breakpoints. Verified independently: ngspice (tight reference) matches this closed form to 3e-16 A away from the corners and the tran_lti oracle gives normalised error 4.9e-13 (ngspice) against 0.40 (RustSpice); with the corners moved onto the grid (td = 1u) RustSpice's error grows to 100 % (1 mA), so the cause is the undamped trapezoid step across a slope discontinuity, not the grid offset.
V1 in 0 PULSE(0 1 1.002u 1u 1u 5u 20u)
C1 in 0 1n
.tran 20n 14u
.end
