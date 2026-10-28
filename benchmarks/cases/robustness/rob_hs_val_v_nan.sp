V source value nan
* bench.id: rob_hs_val_v_nan
* bench.group: robustness
* bench.title: V source value nan
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the unparsable source value 'nan' is dropped and V1 becomes 0 V (v(1) = v(2) = 0 V, i(v1) = 0 A) with exit 0; 'abc' and 'DC abc' behave identically (verified); ngspice-47 exits 1 ('unknown parameter (nan)')
* bench.tags: hostile, value, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: No expression and no truncation: RustSpice drops any unparsable source value ('abc', 'DC abc', 'nan'), ngspice-47 rejects each; section 13.1 files this under lenient-malformed-input (sibling rob_hs_ic_nan_value), not expr-truncation. expect stays loud_error because the value is invalid and ngspice refuses it too.
V1 1 0 nan
R1 1 2 1k
R2 2 0 1k
.op
.end
