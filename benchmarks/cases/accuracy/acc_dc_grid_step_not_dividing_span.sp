DC sweep 0 .. 1 V with a 0.3 V step that does not divide the span: grid 0, 0.3, 0.6, 0.9 (no 1.0)
* bench.id: acc_dc_grid_step_not_dividing_span
* bench.group: accuracy
* bench.title: DC sweep 0 .. 1 V with a 0.3 V step that does not divide the span: grid 0, 0.3, 0.6, 0.9 (no 1.0)
* bench.probes: v(out), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.tags: linear, dc, sweep, grid, step_not_dividing
* bench.note: point count must be 4; tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.dc V1 0 1 0.3
.end
