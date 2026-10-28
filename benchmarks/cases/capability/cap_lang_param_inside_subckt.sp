.param card inside a subcircuit body
* bench.id: cap_lang_param_inside_subckt
* bench.group: capability
* bench.title: .param rk=3k inside the body of .subckt DIV is used by R1 {rk}
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{rk}` (.param inside .subckt not recorded); the loud outcome is incidental to the resistor consumer: with a V source as consumer the same card runs silently (see cap_lang_param_inside_subckt_src)
* bench.tags: linear, op, param, subckt
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked)
.subckt DIV in out
.param rk=3k
R1 in out {rk}
R2 out 0 1k
.ends DIV
V1 in 0 DC 8
X1 in out DIV
.op
.end
