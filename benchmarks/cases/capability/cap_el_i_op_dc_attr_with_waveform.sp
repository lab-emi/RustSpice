Current source with both a DC attribute and a waveform at .op
* bench.id: cap_el_i_op_dc_attr_with_waveform
* bench.group: capability
* bench.title: Current source with both a DC attribute and a waveform at .op
* bench.probes: v(a), v(b), v(c)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.floor: 1e-6
* bench.expect: match
* bench.known_gap: dc-value-ignored-with-waveform
* bench.gap.note: the DC attribute is dropped when a waveform follows it: RustSpice uses the waveform t=0 value (1 V for I1, 1 V for I2) instead of the DC value (4 V and 3 V)
* bench.tags: source, op, waveform, precedence, current_source, silent_risk
* bench.note: Current-source twin of cap_el_v_op_dc_attr_with_waveform. ngspice-47 uses the DC attribute at .op (I1: 4 mA, I2: 3 mA, so 4 V and 3 V) and the t=0 waveform value only when no DC attribute exists (I3: v1 = 1 mA, 1 V; this probe matches in RustSpice and is the control). In .tran both simulators start from the waveform value (see cap_el_v_tran_t0_dc_attr). The DC-over-waveform precedence at .op is a simulator convention that could be checked against ngspice-47 only; independent of it, RustSpice parses the DC attribute and then discards it without a diagnostic
I1 0 a DC 4m SIN(1m 1m 1meg)
R1 a 0 1k
I2 0 b DC 3m PULSE(1m 5m 0 1n 1n 1u 2u)
R2 b 0 1k
I3 0 c PULSE(1m 5m 0 1n 1n 1u 2u)
R3 c 0 1k
.op
.end
