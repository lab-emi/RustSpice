case: 1e308 V across 2 mohm: branch current overflows to infinity
* bench.id: rob_hs_num_overflow_branch_current
* bench.group: robustness
* bench.title: 1e308 V across 2 mohm: branch current overflows to infinity
* bench.probes: v(1), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, numeric, overflow, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: Truth (mpmath): v(1) = 1e308 V, v(2) = 5e307 V, i(v1) = -1e308/2e-3 = -5e310 A, which exceeds DBL_MAX, so no all-finite answer exists and refusal is correct. Not a budget problem: RustSpice equals ngspice whenever the current is representable (V = 1e305 with R = 1e-3, and V = 1e308 with R = 1 ohm, give i(v1) = -5e307 A in both tools) and returns inf from V = 1e306 with R = 1e-3 on (ngspice exits 1 there); the same deck with a diode on node 2 (Newton path) is refused by RustSpice (exit 1, ConvergenceFailure with residual NaN), so the linear path accepts a non-finite solution without a finiteness check.
V1 1 0 1e308
R1 1 2 1e-3
R2 2 0 1e-3
.op
.end
