PULSE with off-grid edges driving an RC filter (tau = 30 steps)
* bench.id: cap_el_v_pulse_unaligned_rc
* bench.group: capability
* bench.title: PULSE with off-grid edges driving an RC filter (tau = 30 steps)
* bench.probes: v(out)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pulse, grid, passive
* bench.note: Loaded twin of cap_el_v_pulse_unaligned (which probes the source node and only tests waveform evaluation). The corners 1.003u, 1.126u, 2.136u and 2.213u are off the 10 ns grid and the RC (tau = 300 ns = 30 steps) integrates across them. A fixed-step integrator that ignores the corners is still accurate here (5e-4 of the swing against the dense-LU/high-order oracle, ngspice as-is 2e-4); the stiff version cap_el_v_pulse_unaligned_rc_stiff shows the loss of accuracy when tau shrinks to one step.
V1 in 0 PULSE(0 5 1.003u 123n 77n 1.01u 3.1u)
R1 in out 1k
C1 out 0 300p
.tran 10n 10u
.end
