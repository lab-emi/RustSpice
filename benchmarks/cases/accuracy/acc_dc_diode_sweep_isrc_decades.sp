Diode driven by a swept current source, 1 nA .. 10 mA in 0.1 mA steps (first step spans 5 decades)
* bench.id: acc_dc_diode_sweep_isrc_decades
* bench.group: accuracy
* bench.title: Diode driven by a swept current source, 1 nA .. 10 mA in 0.1 mA steps (first step spans 5 decades)
* bench.probes: v(d)
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.size: 101
* bench.tags: nonlinear, dc, diode, sweep, isrc
* bench.note: current-source sweep through the diode knee; tol.abs 1e-12 V
I1 0 d 1n
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.dc I1 1n 10m 0.1m
.end
