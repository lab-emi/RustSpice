case: 1 A into a ladder of 1e-3 .. 1e12 ohm resistors (condition ~1e15)
* bench.id: rob_hs_num_condition_chain_1e21
* bench.group: robustness
* bench.title: 1 A into a ladder of 1e-3 .. 1e12 ohm resistors (condition ~1e15)
* bench.probes: v(1), v(2), v(3), v(4)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-15
* bench.oracle: dc_linear
* bench.expect: match
* bench.tags: numeric, condition, dc
* bench.est_rss_mib: 64
I1 0 1 1
R1 1 2 1e9
R2 2 3 1e-3
R3 3 4 1e6
R4 4 0 1e-3
R5 2 0 1e12
R6 3 0 1e3
.op
.end
