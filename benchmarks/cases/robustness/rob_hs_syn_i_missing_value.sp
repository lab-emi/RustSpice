I card without a value (defaults to 0 A)
* bench.id: rob_hs_syn_i_missing_value
* bench.group: robustness
* bench.title: I card without a value (defaults to 0 A)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, syntax, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: I9 is attached to the divider midpoint, so any value RustSpice invented for the missing field (1 A, 1 mA, garbage) would move v(2) away from 2.5 V; a 0 A source and a dropped card are physically identical. Redesigned in calibration: the earlier I9/R9 branch was isolated from every non-zero probe
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
I9 2 0
.op
.end
