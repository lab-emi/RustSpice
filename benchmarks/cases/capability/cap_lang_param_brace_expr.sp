Brace expression in an element value
* bench.id: cap_lang_param_brace_expr
* bench.group: capability
* bench.title: R1 {rv*2} with an expression that starts with an identifier
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{rv*2}`
* bench.tags: linear, op, param
* bench.note: ngspice-47 computes v(out) = 6 V * 1k/(2k+1k) = 2 V (hand-checked)
.param rv=1k
V1 in 0 DC 6
R1 in out {rv*2}
R2 out 0 1k
.op
.end
