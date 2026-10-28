case: 1e-250 A into 1k || 2k: values near the denormal range, v(1) = 6.667e-248
* bench.id: rob_hs_num_scale_current_1e-250
* bench.group: robustness
* bench.title: 1e-250 A into 1k || 2k: values near the denormal range, v(1) = 6.667e-248
* bench.probes: v(1), v(2)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-300
* bench.floor: 1e-300
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, scale, dc
* bench.est_rss_mib: 64
* bench.note: no absolute tolerance may swallow the result: abs tol 1e-300, floor 1e-300
I1 0 1 1e-250
R1 1 0 1k
R2 1 2 1k
R3 2 0 1k
.op
.end
