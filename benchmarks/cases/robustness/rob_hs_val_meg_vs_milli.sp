case: 1MEG = 1e6 but 1M = 1e-3
* bench.id: rob_hs_val_meg_vs_milli
* bench.group: robustness
* bench.title: 1MEG = 1e6 but 1M = 1e-3
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: value, units, parser
* bench.est_rss_mib: 64
* bench.note: R1 = 1 mohm, R2 = 1 Mohm: v(2) ~ 5 V, i(v1) = -5 uA
V1 1 0 5
R1 1 2 1M
R2 2 0 1MEG
.op
.end
