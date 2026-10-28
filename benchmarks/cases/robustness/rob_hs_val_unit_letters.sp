values with unit letters (5V, 1kOhm)
* bench.id: rob_hs_val_unit_letters
* bench.group: robustness
* bench.title: values with unit letters (5V, 1kOhm)
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: hostile, value, units, parser
* bench.est_rss_mib: 64
V1 1 0 5V
R1 1 2 1kOhm
R2 2 0 1k
.op
.end
