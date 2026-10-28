PULSE with all seven arguments, normal and inverted (v1 > v2, negative level)
* bench.id: cap_el_v_pulse_full_args
* bench.group: capability
* bench.title: PULSE with all seven arguments, normal and inverted (v1 > v2, negative level)
* bench.probes: v(a), v(b)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pulse
* bench.note: Both periodic: V1 has td=1u, tr=100n, tf=200n, pw=1u, per=3u; V2 starts high-to-low through zero. The source node is probed so any difference is waveform evaluation, not integration.
V1 a 0 PULSE(0 5 1u 100n 200n 1u 3u)
R1 a 0 1k
V2 b 0 PULSE(1 -1 0.5u 50n 50n 1u 2.5u)
R2 b 0 1k
.tran 10n 10u
.end
