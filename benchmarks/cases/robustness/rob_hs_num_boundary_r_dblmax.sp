5 V across R1 = 1.7e308 ohm in series with 1k (current at the double normal-range boundary)
* bench.id: rob_hs_num_boundary_r_dblmax
* bench.group: robustness
* bench.title: 5 V across R1 = 1.7e308 ohm in series with 1k (current at the double normal-range boundary)
* bench.probes: v(2), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-312
* bench.floor: 1e-310
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, overflow_boundary, denormal, dc
* bench.est_rss_mib: 64
* bench.note: i(v1) = -5/1.7e308 = -2.94e-308 A (just inside the normal range) and v(2) = 2.94e-305 V; both tools agree with the closed form. tol.abs 1e-312 and floor 1e-310 keep the comparison from being swallowed by the default tolerances
V1 1 0 5
R1 1 2 1.7e308
R2 2 0 1k
.op
.end
