R value {1/z} with .param z=0
* bench.id: rob_hs_val_expr_div_zero_param
* bench.group: robustness
* bench.title: R value {1/z} with .param z=0
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: expr-truncation
* bench.gap.note: '{1/z}' is truncated to its leading number 1, so R2 = 1 ohm (v(2) = 4.995 mV) with exit 0, for z = 0 and equally for z = 2 (R2 should be 0.5 ohm); '{2*1k}' gives R = 2 ohm and '{z+1k}' is refused; ngspice-47 exits 1 ('unknown parameter (inf)')
* bench.tags: hostile, value, param, silent_risk
* bench.est_rss_mib: 64
* bench.note: Two independent causes: the truncation to the leading number and a missing finite-value check on parameters (`.param big=1e999` with `R2 2 0 {big}` is accepted by RustSpice with exit 0 and v(2) = 5 V, while ngspice-47 rejects it with the same 'unknown parameter (inf)'; verified). Fixing the truncation alone would still not make RustSpice refuse the division by zero unless the evaluated value is checked for finiteness (resource-budget: non-finite accepted).
.param z=0
V1 1 0 5
R1 1 2 1k
R2 2 0 {1/z}
.op
.end
