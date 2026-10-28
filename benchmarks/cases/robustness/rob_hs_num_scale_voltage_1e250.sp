case: 1e250 V source across a 1k:3k divider, v(2) = 7.5e249
* bench.id: rob_hs_num_scale_voltage_1e250
* bench.group: robustness
* bench.title: 1e250 V source across a 1k:3k divider, v(2) = 7.5e249
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, scale, dc
* bench.est_rss_mib: 64
V1 1 0 1e250
R1 1 2 1k
R2 2 0 3k
.op
.end
