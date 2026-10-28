Several parameters on one card
* bench.id: cap_lang_param_multi_per_card
* bench.group: capability
* bench.title: .param ra=3k rb=1k on one card
* bench.probes: v(out)
* bench.tags: linear, op, param
.param ra=3k rb=1k
V1 in 0 DC 8
R1 in out {ra}
R2 out 0 {rb}
.op
.end
