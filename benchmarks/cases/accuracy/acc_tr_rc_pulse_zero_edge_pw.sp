PULSE tr = tf = 0: pulse-width convention
* bench.id: acc_tr_rc_pulse_zero_edge_pw
* bench.group: accuracy
* bench.title: RC (tau = 1 ns) driven by PULSE(0 1 1n 0 0 2n 5n), tstep 20 ps: where does the trailing edge sit (td+pw or td+tr+pw with tr:=TSTEP)?
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.expect: either
* bench.tags: linear, tran, timing_edge, convention
* bench.est_rss_mib: 16
* bench.note: Ambiguous by construction, so expect=either. An explicit tr = tf = 0 has no SPICE-defined waveform (the tran_lti oracle refuses it). ngspice maps 0 to TSTEP: it ramps over [td, td+TSTEP] and places the trailing edge at td+TSTEP+pw (about half a step late at the leading edge and 1.5 steps late at the trailing edge against the ideal 1-exp(-(t-td)/tau) step; max error 2.8e-2 V). RustSpice keeps an ideal step but, as a fixed-step trapezoid, sees it as a ramp over the step that ENDS at td (half a step early; max error 1.1e-2 V against the ideal step; at t = 1.02 ns v(out) is 0.0295 V vs ideal 0.0198 V vs ngspice 0.0099 V). Neither tool reproduces the ideal curve and they differ from each other by 1.96e-2 V (2.25e-2 of swing) at 1.02 ns. The edge side also depends on floating-point rounding of the accumulated grid time: with tstep 2 ps (acc_tr_rc_pulse_zero_edge) RustSpice reads t = 0.9999999999999999 ns as still low and then matches ngspice to 4e-7.
V1 in 0 PULSE(0 1 1n 0 0 2n 5n)
R1 in out 1k
C1 out 0 1p
.tran 20p 8n
.end
