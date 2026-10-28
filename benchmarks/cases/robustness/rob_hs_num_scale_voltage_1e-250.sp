case: 1e-250 V source across a 1k:3k divider, v(2) = 7.5e-251
* bench.id: rob_hs_num_scale_voltage_1e-250
* bench.group: robustness
* bench.title: 1e-250 V source across a 1k:3k divider, v(2) = 7.5e-251
* bench.probes: v(1), v(2), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-300
* bench.floor: 1e-300
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, scale, dc
* bench.est_rss_mib: 64
V1 1 0 1e-250
R1 1 2 1k
R2 2 0 3k
.op
.end
