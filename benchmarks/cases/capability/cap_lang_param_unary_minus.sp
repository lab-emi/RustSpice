Unary minus in expressions
* bench.id: cap_lang_param_unary_minus
* bench.group: capability
* bench.title: V1 {-vp} and R {-(-1k)} exercise unary minus
* bench.probes: v(in), v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{-(-1k)}` at the R card; the preceding `V1 in 0 DC {-vp}` is already read silently as 0 V (RustSpice v(in) = 0 V where ngspice gives -5 V)
* bench.tags: linear, op, param
* bench.note: ngspice-47 computes v(in) = -5 V and v(out) = -5 V * 1k/(1k+1k) = -2.5 V (hand-checked)
.param vp=5
V1 in 0 DC {-vp}
R1 in out {-(-1k)}
R2 out 0 1k
.op
.end
