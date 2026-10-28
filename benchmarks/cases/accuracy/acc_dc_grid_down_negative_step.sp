DC sweep 1 V .. -1 V with a -0.25 V step, crossing zero
* bench.id: acc_dc_grid_down_negative_step
* bench.group: accuracy
* bench.title: DC sweep 1 V .. -1 V with a -0.25 V step, crossing zero
* bench.probes: v(out), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.tags: linear, dc, sweep, grid, sweep_down
* bench.note: 9 points; tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.dc V1 1 -1 -0.25
.end
