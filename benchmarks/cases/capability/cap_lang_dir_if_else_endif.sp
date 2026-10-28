Conditional netlist block
* bench.id: cap_lang_dir_if_else_endif
* bench.group: capability
* bench.title: .if (sel == 1) / .else / .endif keeps exactly one of two resistors (sel = 2 keeps the 3k branch)
* bench.probes: v(out)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .if/.else/.endif are ignored like an unknown card: both branches are instantiated in parallel (1k || 3k) and v(out) = 4.571 V instead of 2 V, exit 0, no message
* bench.tags: linear, op, param, conditional, silent_risk
* bench.note: ngspice-47 evaluates the condition (v(out) = 8 V * 1k/(3k+1k) = 2 V, hand-checked); the branches use different element names (R1, R3) so duplicate-name handling is not involved
.param sel=2
V1 in 0 DC 8
.if (sel == 1)
R1 in out 1k
.else
R3 in out 3k
.endif
R2 out 0 1k
.op
.end
