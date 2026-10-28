Diode behind 1k swept downwards 5 .. 0 V in -1 mV steps
* bench.id: acc_dc_grid_drift_down_diode
* bench.group: accuracy
* bench.title: Diode (IS=1e-14) behind 1k, source swept 5 V down to 0 V in -1 mV steps: 5001 points, end points 5.000 V and 0.000 V
* bench.probes: v(d), i(v1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.size: 5001
* bench.tags: nonlinear, dc, diode, sweep, sweep_down, grid, many_points
* bench.note: decreasing sweep with a negative step and a nonlinear device; once the count matches the end points are probed with values (at 5 V v(d) = 0.693 V, i = -4.31 mA; at 0 V both are 0) but the engine skips the value comparison while the count differs; the first 5000 points agree with tight ngspice to 7e-10 V (the diode solve tolerance) and their sweep values are bit-identical in the two tools; RustSpice's last written point is 0.0009999999999958991 V, ngspice writes a 5001st point at -4.1e-15 V (measured); the 5001st accumulated value (ngspice's, -4.1e-15 V) lies 4.1e-15 V beyond the end point, more than the 1e-12*step = 1e-15 V guard of RustSpice's sweep loop (SUITE_SPEC 14.1), which is why the point is dropped
V1 in 0 5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.dc V1 5 0 -1m
.end
