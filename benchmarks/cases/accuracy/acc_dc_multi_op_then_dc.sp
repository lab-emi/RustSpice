An .op followed by a .dc sweep: both analyses must be reported correctly (control for the reverse order)
* bench.id: acc_dc_multi_op_then_dc
* bench.group: accuracy
* bench.title: An .op followed by a .dc sweep: both analyses must be reported correctly (control for the reverse order)
* bench.probes: v(out)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.tags: linear, op, dc, multi_analysis, control
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 in 0 3
R1 in out 1k
R2 out 0 1k
.op
.dc V1 0 1 0.5
.end
