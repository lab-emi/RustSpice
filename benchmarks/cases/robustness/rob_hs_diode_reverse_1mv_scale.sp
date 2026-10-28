case: 1 MV reverse bias on a diode (exp underflow)
* bench.id: rob_hs_diode_reverse_1mv_scale
* bench.group: robustness
* bench.title: 1 MV reverse bias on a diode (exp underflow)
* bench.probes: v(2), i(v1)
* bench.oracle: dc_nonlinear
* bench.expect: match
* bench.tags: hostile, nonlinear, diode, underflow
* bench.est_rss_mib: 64
V1 1 0 -1e6
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=1e-14)
.op
.end
