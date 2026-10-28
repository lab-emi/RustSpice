Reference to an undefined parameter
* bench.id: cap_lang_param_undefined
* bench.group: capability
* bench.title: R1 {nope} names a parameter that is never defined
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, param, invalid_input, ngspice_error_ok
V1 in 0 DC 6
R1 in out {nope}
R2 out 0 1k
.op
.end
