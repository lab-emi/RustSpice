Spaces around the equals sign of .param
* bench.id: cap_lang_param_spaced_equals
* bench.group: capability
* bench.title: .param ra = 3k (spaces around '=') defines ra
* bench.probes: v(out)
* bench.tags: linear, op, param, syntax
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(3k+1k) = 2 V (hand-checked)
.param ra = 3k
V1 in 0 DC 8
R1 in out {ra}
R2 out 0 1k
.op
.end
