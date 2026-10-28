MOSFET with drain, gate, source and bulk on the same node
* bench.id: rob_hs_mos_all_terminals_same_node
* bench.group: robustness
* bench.title: MOSFET with drain, gate, source and bulk on the same node
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: nonlinear, mos, dc, degenerate
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
M9 2 2 2 2 NM W=10u L=1u
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
