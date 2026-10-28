M card with three nodes and no model
* bench.id: rob_hs_syn_m_too_few_nodes
* bench.group: robustness
* bench.title: M card with three nodes and no model
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
M9 2 1 0
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
