Subcircuit without .ends
* bench.id: cap_lang_sub_missing_ends
* bench.group: capability
* bench.title: .subckt FOO is opened after a complete top-level circuit and never closed by .ends
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: unclosed .subckt is accepted and swallows every later card including the .op, so the implicit default .op runs: v(out) = 4.0 V, exit 0, nothing on stderr (ngspice: 'Mismatch of .subckt ... .ends statements', exit 1)
* bench.tags: linear, op, subckt, invalid_input, ngspice_error_ok
* bench.note: the top-level circuit precedes the .subckt, so the numbers equal the valid deck (8*1k/2k = 4.0 V) and this deck alone shows no wrong number; the harm is the swallowed analysis, shown by cap_lang_sub_missing_ends_dc (a .dc after the unclosed .subckt runs as a one-point op instead of 5 points). Cards after an unclosed .subckt are dropped: with R1/R2 after it, v(out) is absent and the exit code is 0
V1 in 0 DC 8
R1 in out 1k
R2 out 0 1k
.subckt FOO a b
R3 a b 1k
.op
.end
