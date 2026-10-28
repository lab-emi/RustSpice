Operating point of PULSE(2 5 ...) DC 1: an explicit DC value overrides the t = 0 value of the waveform
* bench.id: acc_dc_op_dc_value_overrides_pulse
* bench.group: accuracy
* bench.title: Operating point of PULSE(2 5 ...) DC 1: an explicit DC value overrides the t = 0 value of the waveform
* bench.probes: v(c), i(v3)
* bench.expect: match
* bench.known_gap: dc-value-ignored-with-waveform
* bench.gap.note: the explicit DC 1 is ignored: RS reports v(c) = 2 V (the pulse initial value) instead of 1 V (silent)
* bench.tags: linear, op, source_waveform, silent_risk
* bench.note: ngspice-47 .op uses the explicit DC attribute (v(c) = 1 V, i(v3) = -1 mA) instead of the PULSE t = 0 value (2 V); RustSpice reports 2 V and -2 mA whether the DC token is placed before or after the waveform, and equals ngspice once `DC 1` is removed. The DC-over-waveform precedence at .op is a simulator convention that could be checked against ngspice-47 only; independent of it, RustSpice parses the DC attribute and then discards it without a diagnostic
V3 c 0 PULSE(2 5 0 1n 1n 1u 3u) DC 1
R3 c 0 1k
.op
.end
