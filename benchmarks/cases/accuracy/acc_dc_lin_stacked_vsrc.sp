Stacked (floating) 5 V + 5 V sources into a linear divider: control for the clamp-rail decks
* bench.id: acc_dc_lin_stacked_vsrc
* bench.group: accuracy
* bench.title: Stacked (floating) 5 V + 5 V sources into a linear divider: control for the clamp-rail decks
* bench.probes: v(1), v(2), v(3), i(v1), i(v2)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.oracle: dc_linear
* bench.tags: linear, op, stacked_source, control
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 1 0 5
V2 2 1 5
R1 2 3 1k
R2 3 0 1k
.op
.end
