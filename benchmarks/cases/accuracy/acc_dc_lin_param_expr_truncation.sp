Parameter expression value {2*rv} for a resistor: R1 = 2 kohm
* bench.id: acc_dc_lin_param_expr_truncation
* bench.group: accuracy
* bench.title: Parameter expression value {2*rv} for a resistor: R1 = 2 kohm
* bench.probes: v(out), i(v1)
* bench.expect: match
* bench.tags: linear, op, parse, expression, silent_risk
* bench.note: tol.rel/tol.abs default; value 1.6666667 V from the closed form 5*1k/3k
.param rv=1k
V1 in 0 5
R1 in out {2*rv}
R2 out 0 1k
.op
.end
