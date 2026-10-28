Wheatstone bridge with a 1 uohm balancing arm
* bench.id: rob_hs_num_condition_bridge_1e-6
* bench.group: robustness
* bench.title: Wheatstone bridge with a 1 uohm balancing arm
* bench.probes: v(1), v(2), v(3), i(v1)
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, condition, dc
* bench.est_rss_mib: 64
* bench.note: exact: v(2)=v(3)=0.5 V, i(v1)=-1 A (the 1 uohm arm carries no current)
V1 1 0 1
R1 1 2 1
R2 1 3 1
R3 2 0 1
R4 3 0 1
R5 2 3 1e-6
.op
.end
