R value {log(0)}
* bench.id: rob_hs_val_expr_log_zero
* bench.group: robustness
* bench.title: R value {log(0)}
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, value, param, nonfinite
* bench.est_rss_mib: 64
* bench.note: ngspice rejects log(0) (-inf). RustSpice also refuses, but for an unrelated reason: it refuses EVERY braced function expression ({log(10)*1k} too, which ngspice evaluates to 2.3026k), so this pass does not show that non-finite expression results are detected
V1 1 0 5
R1 1 2 1k
R2 2 0 {log(0)}
.op
.end
