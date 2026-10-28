Transient start: the t=0 waveform value, not the DC attribute, seeds the initial state
* bench.id: cap_el_v_tran_t0_dc_attr
* bench.group: capability
* bench.title: Transient start: the t=0 waveform value, not the DC attribute, seeds the initial state
* bench.probes: v(b), v(d), v(f)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.tags: source, tran, waveform, precedence, passive
* bench.note: Complement of cap_el_v_op_dc_attr_with_waveform: at .op ngspice uses the DC attribute, but the initial operating point of a .tran starts from the waveform's t=0 value (SIN: vo, PULSE: v1). The RC nodes b, d and f therefore start at their waveform-driven steady state (1 V, 0.5 V, 1 V) and not at the DC attribute (2 V, 4 V, 3 V); a simulator that seeded the transient with the DC value would show a 1-3 V decay at the start of each trace.
V1 a 0 DC 2 SIN(1 0.2 1meg)
R1 a b 1k
C1 b 0 1n
V2 c 0 DC 4 PULSE(0.5 3 1u 10n 10n 1u 3u)
R2 c d 1k
C2 d 0 1n
I1 0 e DC 3m PULSE(1m 4m 1u 10n 10n 1u 3u)
R3 e f 1
R4 f 0 1k
C3 f 0 1n
.tran 10n 3u
.end
