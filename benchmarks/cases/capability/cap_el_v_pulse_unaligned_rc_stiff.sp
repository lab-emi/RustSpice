PULSE with off-grid edges driving a stiff RC filter (tau = 1 step)
* bench.id: cap_el_v_pulse_unaligned_rc_stiff
* bench.group: capability
* bench.title: PULSE with off-grid edges driving a stiff RC filter (tau = 1 step)
* bench.probes: v(out)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pulse, grid, passive, silent_risk
* bench.note: Same circuit as cap_el_v_pulse_unaligned_rc with C = 10p, so tau = R*C = 10 ns equals the step. The reference (ngspice tight, tmax 0.5 ns) and the independent oracle agree with each other to 1e-3, RustSpice differs from both by 8e-3 of the swing (the error is largest after the corner at 8.3 us); ngspice as-is is at 1.5e-3 because it places time points on the corners. Flip tests: the same deck with .tran 1n 10u passes at 4e-5 (so the cause is the step size, not the netlist or the oracle). With the corners moved onto the 10 ns grid RustSpice is still at 4.3e-3, but ngspice as-is is then at 2.7e-3, the same size, so only the off-grid configuration is a distinguishing gap.
V1 in 0 PULSE(0 5 1.003u 123n 77n 1.01u 3.1u)
R1 in out 1k
C1 out 0 10p
.tran 10n 10u
.end
