OP value of a source with both a DC attribute and a waveform
* bench.id: cap_el_v_op_dc_attr_with_waveform
* bench.group: capability
* bench.title: OP value of a source with both a DC attribute and a waveform
* bench.probes: v(a), v(b), v(c), v(d)
* bench.expect: match
* bench.known_gap: dc-value-ignored-with-waveform
* bench.gap.note: the DC attribute is parsed and then dropped when a waveform follows it: V1 and V3 give their waveform t=0 value (1 V and 0.5 V) instead of the DC value 2 V
* bench.tags: source, op, waveform, precedence, silent_risk
* bench.note: ngspice-47 (tight reference) uses the DC attribute at .op when present (V1 and V3: 2 V) and the t=0 waveform value otherwise (V2: 1 V, V4: 0.5 V; these two probes are the controls and match). Minimal flip: deleting `DC 2` from V1 makes RustSpice agree with ngspice (1 V). In .tran both simulators start from the waveform t=0 value, not the DC attribute (see cap_el_v_tran_t0_dc_attr), so only .op/.dc are affected. The DC-over-waveform precedence at .op is a simulator convention that could be checked against ngspice-47 only; independent of it, RustSpice parses the DC attribute and then discards it without a diagnostic
V1 a 0 DC 2 SIN(1 1 1meg)
V2 b 0 SIN(1 1 1meg)
V3 c 0 DC 2 PULSE(0.5 3 0 1n 1n 1u 2u)
V4 d 0 PULSE(0.5 3 0 1n 1n 1u 2u)
R1 a 0 1k
R2 b 0 1k
R3 c 0 1k
R4 d 0 1k
.op
.end
