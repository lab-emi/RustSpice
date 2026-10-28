Parameter names are case-insensitive
* bench.id: cap_lang_param_case_insensitive
* bench.group: capability
* bench.title: .PARAM Ra=3k referenced as {rA}
* bench.probes: v(out)
* bench.tags: linear, op, param, syntax, case
.PARAM Ra=3k
V1 in 0 DC 8
R1 in out {rA}
R2 out 0 1k
.op
.end
