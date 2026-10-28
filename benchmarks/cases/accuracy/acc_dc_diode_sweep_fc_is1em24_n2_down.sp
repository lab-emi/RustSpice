IS=1e-24 N=2 diode behind 100 ohm, source swept 12 V .. 0 V in -1 V steps (decreasing)
* bench.id: acc_dc_diode_sweep_fc_is1em24_n2_down
* bench.group: accuracy
* bench.title: IS=1e-24 N=2 diode behind 100 ohm, source swept 12 V .. 0 V in -1 V steps (decreasing)
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-17
* bench.floor: 1e-14
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, dc, diode, sweep, sweep_down, small_is, silent_risk
* bench.note: tol.abs 1e-17 A for the sub-pA end of the sweep
V1 in 0 12
R1 in d 100
D1 d 0 dm
.model dm D(IS=1e-24 N=2)
.dc V1 12 0 -1
.end
