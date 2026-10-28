DC sweep 0 .. 1 V in 1 mV steps (1001 points): step accumulation must not drop or add the last point
* bench.id: acc_dc_grid_many_points_1001
* bench.group: accuracy
* bench.title: DC sweep 0 .. 1 V in 1 mV steps (1001 points): step accumulation must not drop or add the last point
* bench.probes: v(out), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.size: 1001
* bench.tags: linear, dc, sweep, grid, many_points
* bench.note: 1001 points; tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.dc V1 0 1 1m
.end
