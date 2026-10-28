Subcircuit without .ends before a .dc card
* bench.id: cap_lang_sub_missing_ends_dc
* bench.group: capability
* bench.title: unclosed .subckt FOO followed by a .dc sweep: the sweep is swallowed
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: unclosed .subckt swallows the .dc card: RustSpice runs the implicit .op (one point, v(out) = 4.0 V) instead of the 5-point sweep, exit 0, nothing on stderr (ngspice: 'Mismatch of .subckt ... .ends statements', exit 1)
* bench.tags: linear, dc, subckt, invalid_input, ngspice_error_ok
* bench.note: the closed variant (`.ends` after R3) gives the 5-point sweep v(out) = 0, 1, 2, 3, 4 V (V1 = 0 to 8 V in steps of 2, divider 1k/1k) in RustSpice and ngspice alike; without `.ends` the analysis card is lost silently. Companion of cap_lang_sub_missing_ends (the .op case, where the dropped card is harmless)
V1 in 0 DC 8
R1 in out 1k
R2 out 0 1k
.subckt FOO a b
R3 a b 1k
.dc V1 0 8 2
.end
