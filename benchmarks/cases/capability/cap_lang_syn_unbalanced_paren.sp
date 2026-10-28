Waveform argument list without the closing parenthesis
* bench.id: cap_lang_syn_unbalanced_paren
* bench.group: capability
* bench.title: PULSE(0 5 0 1n 1n 1u 2u is missing its closing parenthesis; ngspice still reads it as a pulse
* bench.probes: v(in)
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: the argument list is not found without its ')' and the source silently becomes DC 0 V: v(in) stays 0 V instead of rising to 5 V (exit 0, no message)
* bench.tags: linear, tran, syntax, invalid_input, waveform, silent_risk
* bench.note: decisive: ngspice-47 reads the unclosed list as PULSE(0 5 0 1n 1n 1u 2u) (v(in) = 0.05 V at 10 ps, 5 V from 1 ns to 1 us, hand-checked) while the identical closed-paren source works in RustSpice (cap_lang_syn_cont_pulse), so only the missing ')' makes the waveform vanish; refusing loudly would be a smaller defect (re-tag unsupported-syntax-loud) and reading the pulse closes the gap
V1 in 0 PULSE(0 5 0 1n 1n 1u 2u
R1 in 0 1k
.tran 10n 1u
.end
