dc V1 1 3 5 (single point at the start)
* bench.id: rob_hs_dc_step_larger_than_range
* bench.group: robustness
* bench.title: .dc V1 1 3 5 (step larger than the range: a single point at the start)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: dc, args, lenient_ok
* bench.est_rss_mib: 64
* bench.note: both tools produce exactly one point at V1 = 1 V (v(2)=0.5 V, i(v1)=-0.5 mA); the old deck swept 0..1 where every probe is 0 and was vacuous
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 1 3 5
.end
