IS=1e-18 diode behind 1k, source swept 5 V .. 0 V in -0.5 V steps (decreasing)
* bench.id: acc_dc_diode_sweep_fc_is1em18_down
* bench.group: accuracy
* bench.title: IS=1e-18 diode behind 1k, source swept 5 V .. 0 V in -0.5 V steps (decreasing)
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-17
* bench.floor: 1e-14
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, dc, diode, sweep, sweep_down, small_is, silent_risk
* bench.note: tol.abs 1e-17 A for the sub-pA end of the sweep
V1 in 0 5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-18 N=1)
.dc V1 5 0 -0.5
.end
