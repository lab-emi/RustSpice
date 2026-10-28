Parameters defined from other parameters
* bench.id: cap_lang_param_nested
* bench.group: capability
* bench.title: .param a=1k b={a*2} c={a+b} chains three definitions (c = 3k)
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{c}` (b={a*2} and c={a+b} are not recorded as parameters); with a V source as consumer (.param b={a*2}, V1 in 0 DC {b}) the same chain runs silently: exit 0, v(in) = 0 V where ngspice gives 2000 V
* bench.tags: linear, op, param
* bench.note: ngspice-47 computes c = 1k + 2k = 3k and v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked)
.param a=1k
.param b={a*2}
.param c={a+b}
V1 in 0 DC 8
R1 in out {c}
R2 out 0 1k
.op
.end
