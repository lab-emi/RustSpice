Parameter defined twice
* bench.id: cap_lang_param_redefine
* bench.group: capability
* bench.title: .param rv=1k then .param rv=3k: the reference simulator's choice is the oracle
* bench.probes: v(out)
* bench.tags: linear, op, param
.param rv=1k
.param rv=3k
V1 in 0 DC 8
R1 in out {rv}
R2 out 0 1k
.op
.end
