Diode with large saturation current IS=1e-9, 5 V / 1k
* bench.id: rob_cv_diode_is1e9_op
* bench.group: robustness
* bench.title: Diode with large saturation current IS=1e-9, 5 V / 1k
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, op
* bench.note: Regression anchor (large IS).
V1 1 0 5
R1 1 2 1k
D1 2 0 dm
.model dm D(is=1e-9 n=1)
.op
.end
