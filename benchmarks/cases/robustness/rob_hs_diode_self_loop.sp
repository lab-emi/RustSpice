diode with anode and cathode on the same node
* bench.id: rob_hs_diode_self_loop
* bench.group: robustness
* bench.title: diode with anode and cathode on the same node
* bench.probes: v(1), v(2), i(v1)
* bench.oracle: dc_nonlinear
* bench.expect: match
* bench.tags: nonlinear, diode, dc, degenerate
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
D9 2 2 DM
.model DM D(IS=1e-14)
.op
.end
