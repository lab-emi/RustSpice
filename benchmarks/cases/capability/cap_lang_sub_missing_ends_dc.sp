Subcircuit without .ends before a .dc card
* bench.id: cap_lang_sub_missing_ends_dc
* bench.group: capability
* bench.title: unclosed .subckt FOO followed by a .dc sweep: the sweep is swallowed
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, dc, subckt, invalid_input, ngspice_error_ok
* bench.note: the closed variant (`.ends` after R3) gives the 5-point sweep v(out) = 0, 1, 2, 3, 4 V (V1 = 0 to 8 V in steps of 2, divider 1k/1k) in RustSpice and ngspice alike; without `.ends` the analysis card is lost silently. Companion of cap_lang_sub_missing_ends (the .op case, where the dropped card is harmless) [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 8
R1 in out 1k
R2 out 0 1k
.subckt FOO a b
R3 a b 1k
.dc V1 0 8 2
.end
