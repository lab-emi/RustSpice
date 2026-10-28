I source value inf
* bench.id: rob_hs_val_i_inf
* bench.group: robustness
* bench.title: I source value inf
* bench.probes: v(1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the unparsable source value 'inf' is dropped and I1 becomes 0 A (v(1) = 0 V) with exit 0; 'abc' behaves identically (verified); ngspice-47 exits 1 ('unknown parameter (inf)')
* bench.tags: hostile, value, nonfinite, silent_risk
* bench.est_rss_mib: 64
* bench.note: No expression and no truncation: RustSpice drops any unparsable source value ('abc' as well as 'inf'), ngspice-47 rejects each; section 13.1 files this under lenient-malformed-input (sibling rob_hs_ic_nan_value), not expr-truncation. expect stays loud_error because the value is invalid and ngspice refuses it too.
I1 0 1 inf
R1 1 0 1k
.op
.end
