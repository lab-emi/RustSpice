R value {1e308*10} overflows inside an expression
* bench.id: rob_hs_val_expr_overflow
* bench.group: robustness
* bench.title: R value {1e308*10} overflows inside an expression
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, value, overflow, param
* bench.est_rss_mib: 64
* bench.note: the output (v(2)=5 V) is the same as an expression that evaluated to infinity, but RustSpice does not evaluate it: {2*1k} gives R=2 ohm instead of 2k (verified), so the acceptance is the expr-truncation defect and not an overflow check A second, independent cause: RustSpice has no finite-value check on parameters either (`.param big=1e999` with `R2 2 0 {big}` is accepted, v(2) = 5 V, exit 0, while ngspice-47 rejects it; verified), and the finite {1e300*10} gives the same numbers in both tools, so a closing fix needs expression evaluation and a finiteness check on the result. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
R2 2 0 {1e308*10}
.op
.end
