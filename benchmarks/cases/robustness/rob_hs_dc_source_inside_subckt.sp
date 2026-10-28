dc of a source that lives inside a subcircuit (by its local name)
* bench.id: rob_hs_dc_source_inside_subckt
* bench.group: robustness
* bench.title: .dc of a source that lives inside a subcircuit (by its local name)
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: hostile, dc, args, subckt
* bench.est_rss_mib: 64
.subckt s a b
V1 a b 1
R1 a b 1k
.ends
X1 1 0 s
R2 1 0 1k
.dc V1 0 3 1
.end
