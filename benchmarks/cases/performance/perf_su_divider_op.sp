Four-resistor divider network, single operating point
* bench.id: perf_su_divider_op
* bench.group: performance
* bench.title: Four-resistor divider network, single operating point
* bench.probes: v(mid), v(out), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 4
* bench.est_rss_mib: 10
* bench.tags: startup, linear, op
* bench.note: five-element linear .op: the run is process start-up, parse and setup; the solve itself is negligible
V1 in 0 5
R1 in mid 1k
R2 mid 0 2k
R3 mid out 3.3k
R4 out 0 4.7k
.op
.end
