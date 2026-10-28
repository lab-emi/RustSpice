V source value 1e999 (infinity)
* bench.id: rob_hs_val_v_1e999
* bench.group: robustness
* bench.title: V source value 1e999 (infinity)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, value, nonfinite, overflow, silent_risk
* bench.est_rss_mib: 64
* bench.note: ngspice-47 refuses 1e999, 1e309 and 1e308k (suffix overflow) alike, and RustSpice returns NaN with exit 0 for all three; 1e300 and 5 give identical finite results in both tools (v(2) = V/2). The same source with a diode on node 2 (Newton path) is refused by RustSpice (exit 1, ConvergenceFailure with residual NaN; deck rob_hs_val_v_1e999_diode), so the defect is the missing finiteness check on the linear solve path, not a resource budget. The literals inf and nan are a different defect (dropped as unparsable, V1 = 0 V; see rob_hs_val_v_nan, rob_hs_val_i_inf).
V1 1 0 1e999
R1 1 2 1k
R2 2 0 1k
.op
.end
