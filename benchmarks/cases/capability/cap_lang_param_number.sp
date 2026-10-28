Numeric parameter in an element value
* bench.id: cap_lang_param_number
* bench.group: capability
* bench.title: .param rv=2k used as R1 {rv} in a divider (rv = 2k)
* bench.probes: v(out), i(v1)
* bench.oracle: dc_linear
* bench.tags: linear, op, param
.param rv=2k
V1 in 0 DC 6
R1 in out {rv}
R2 out 0 1k
.op
.end
