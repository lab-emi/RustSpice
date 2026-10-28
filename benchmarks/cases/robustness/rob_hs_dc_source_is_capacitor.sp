dc of a capacitor
* bench.id: rob_hs_dc_source_is_capacitor
* bench.group: robustness
* bench.title: .dc of a capacitor
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, dc, args
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
C1 2 0 1n
.dc C1 1n 3n 1n
.end
