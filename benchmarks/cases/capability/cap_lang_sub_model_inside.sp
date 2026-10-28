Model defined inside a subcircuit
* bench.id: cap_lang_sub_model_inside
* bench.group: capability
* bench.title: .model DM D(IS=1e-12) inside .subckt applies to the diode in that subcircuit
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .model inside .subckt is dropped; the diode silently uses default parameters (0.693 V instead of 0.574 V)
* bench.tags: nonlinear, op, subckt, model_scope
.subckt CLAMP in out
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-12)
.ends CLAMP
V1 in 0 DC 5
X1 in out CLAMP
.op
.end
