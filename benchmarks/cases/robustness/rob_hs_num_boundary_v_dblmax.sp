1e308 V source across a 1k:3k divider (values near the double overflow limit)
* bench.id: rob_hs_num_boundary_v_dblmax
* bench.group: robustness
* bench.title: 1e308 V source across a 1k:3k divider (values near the double overflow limit)
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, overflow_boundary, dc
* bench.est_rss_mib: 64
* bench.note: v(2) = 7.5e307 V and i(v1) = -2.5e304 A are finite and exact (1e308*3/4); both tools agree, nothing overflows (contrast with num_overflow_branch_current, where i = 1e308/2e-3 does)
V1 1 0 1e308
R1 1 2 1k
R2 2 0 3k
.op
.end
