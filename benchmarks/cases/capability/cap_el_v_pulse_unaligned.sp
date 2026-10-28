PULSE edges that do not fall on the time grid
* bench.id: cap_el_v_pulse_unaligned
* bench.group: capability
* bench.title: PULSE edges that do not fall on the time grid
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pulse, grid
* bench.note: td, tr, tf, pw and per are all off the 10 ns grid; each sample is an exact function evaluation in both simulators. The source node is probed directly, so this tests waveform evaluation only: RustSpice is at 6e-13 of the oracle and of the converged reference (it passes legitimately; an earlier claim of a 2.5e-2 fixed-step error does not reproduce on this deck). Fixed-step integration across the same off-grid corners is tested by cap_el_v_pulse_unaligned_rc (tau = 30 steps, passes) and cap_el_v_pulse_unaligned_rc_stiff (tau = 1 step, known gap).
V1 in 0 PULSE(0 5 1.003u 123n 77n 1.01u 3.1u)
R1 in 0 1k
.tran 10n 10u
.end
