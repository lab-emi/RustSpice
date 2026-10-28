PULSE spelling: space before paren, lower case, commas, params, inner spaces, DC 0 prefix
* bench.id: cap_el_v_pulse_syntax_forms
* bench.group: capability
* bench.title: PULSE spelling: space before paren, lower case, commas, params, inner spaces, DC 0 prefix
* bench.probes: v(a), v(b), v(c), v(d), v(e), v(f)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: source, tran, waveform, pulse, parser
* bench.note: All six sources are the same waveform.
.param vh=5 tdel=1u
V1 a 0 PULSE (0 5 1u 100n 100n 1u 3u)
v2 b 0 pulse(0 5 1u 100n 100n 1u 3u)
V3 c 0 PULSE(0,5,1u,100n,100n,1u,3u)
V4 d 0 PULSE(0 {vh} {tdel} 100n 100n 1u 3u)
V5 e 0 PULSE( 0 5 1u 100n 100n 1u 3u )
V6 f 0 DC 0 PULSE(0 5 1u 100n 100n 1u 3u)
R1 a 0 1k
R2 b 0 1k
R3 c 0 1k
R4 d 0 1k
R5 e 0 1k
R6 f 0 1k
.tran 10n 6u
.end
