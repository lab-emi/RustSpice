1 nohm / 1 nohm divider on 1 V (conductances of 1e9 S)
* bench.id: rob_cv_res_1n_divider_op
* bench.group: robustness
* bench.title: 1 nohm / 1 nohm divider on 1 V (conductances of 1e9 S)
* bench.probes: v(2), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, small_resistance, op
* bench.note: Regression anchor: i(v1)=-5e8 A. Exact 0.5 V in both.
V1 1 0 1
R1 1 2 1n
R2 2 0 1n
.op
.end
