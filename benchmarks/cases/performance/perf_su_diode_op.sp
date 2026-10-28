Single diode with series resistor, one operating point
* bench.id: perf_su_diode_op
* bench.group: performance
* bench.title: Single diode with series resistor, one operating point
* bench.probes: v(d), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-9
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 3
* bench.est_rss_mib: 10
* bench.tags: startup, nonlinear, diode, op
* bench.note: one Newton solve of a 3-unknown diode circuit; start-up and parse dominate, plus any per-process compile cost of the nonlinear kernel
.model dm D(IS=1e-14 N=1)
V1 in 0 5
R1 in d 1k
D1 d 0 dm
.op
.end
