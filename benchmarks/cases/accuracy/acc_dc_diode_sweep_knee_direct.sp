Diode IS=1e-14 directly across the swept source, 0.50 .. 0.80 V in 2 mV steps: current spans 5 decades
* bench.id: acc_dc_diode_sweep_knee_direct
* bench.group: accuracy
* bench.title: Diode IS=1e-14 directly across the swept source, 0.50 .. 0.80 V in 2 mV steps: current spans 5 decades
* bench.probes: i(v1)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.size: 151
* bench.tags: nonlinear, dc, diode, sweep, steep_exponential
* bench.note: i(v1) from 2.5e-6 A to 0.27 A; tol.abs 1e-18 A
V1 in 0 0.5
D1 in 0 dm
.model dm D(IS=1e-14 N=1)
.dc V1 0.5 0.8 0.002
.end
