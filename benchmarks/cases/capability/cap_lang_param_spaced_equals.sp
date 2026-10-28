Spaces around the equals sign of .param
* bench.id: cap_lang_param_spaced_equals
* bench.group: capability
* bench.title: .param ra = 3k (spaces around '=') defines ra
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: .param ra = 3k is not recorded, so {ra} is an Invalid resistance value; with a V source as consumer the same card runs silently (see cap_lang_param_spaced_equals_src)
* bench.tags: linear, op, param, syntax
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked)
.param ra = 3k
V1 in 0 DC 8
R1 in out {ra}
R2 out 0 1k
.op
.end
