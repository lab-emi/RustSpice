V source value nan
* bench.id: rob_hs_val_v_nan
* bench.group: robustness
* bench.title: V source value nan
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, value, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: No expression and no truncation: RustSpice drops any unparsable source value ('abc', 'DC abc', 'nan'), ngspice-47 rejects each; section 13.1 files this under lenient-malformed-input (sibling rob_hs_ic_nan_value), not expr-truncation. expect stays loud_error because the value is invalid and ngspice refuses it too. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 nan
R1 1 2 1k
R2 2 0 1k
.op
.end
