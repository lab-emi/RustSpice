diode model with N=0 (division by zero in the thermal voltage)
* bench.id: rob_hs_diode_n_zero
* bench.group: robustness
* bench.title: diode model with N=0 (division by zero in the thermal voltage)
* bench.probes: v(2)
* bench.expect: loud_error
* bench.tags: hostile, nonlinear, diode, model, div_zero
* bench.est_rss_mib: 64
V1 1 0 5
R1 1 2 1k
D1 2 0 DM
.model DM D(IS=1e-14 N=0)
.op
.end
