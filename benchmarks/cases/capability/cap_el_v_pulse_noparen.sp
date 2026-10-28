PULSE written without parentheses
* bench.id: cap_el_v_pulse_noparen
* bench.group: capability
* bench.title: PULSE written without parentheses
* bench.probes: v(in)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.known_gap: silent-ignore-source-waveform
* bench.gap.note: without parentheses the whole waveform is dropped and the source becomes DC 0 V, no diagnostic
* bench.tags: source, tran, waveform, pulse, parser, silent_risk
* bench.note: ngspice accepts the parenthesis-free form. Verified: ngspice-47 equals the PULSE(0 5 1u 100n 100n 1u 3u) closed form to 1e-14 V; RustSpice stays at 0 V (no parentheses found, first token PULSE is not a number). The parenthesis-free form is an ngspice leniency that no other simulator could confirm; a silent DC 0 V is wrong under either reading (reading the waveform or refusing loudly). The same card with parentheses is read correctly by RustSpice (3.95e-13 of swing against ngspice)
V1 in 0 PULSE 0 5 1u 100n 100n 1u 3u
R1 in 0 1k
.tran 10n 6u
.end
