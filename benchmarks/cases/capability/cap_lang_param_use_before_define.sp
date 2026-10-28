Parameter used before its .param card
* bench.id: cap_lang_param_use_before_define
* bench.group: capability
* bench.title: R1 {rv} appears before .param rv=3k
* bench.probes: v(out)
* bench.tags: linear, op, param
V1 in 0 DC 8
R1 in out {rv}
R2 out 0 1k
.param rv=3k
.op
.end
