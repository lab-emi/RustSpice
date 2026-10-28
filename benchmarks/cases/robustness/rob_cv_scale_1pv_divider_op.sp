1 pV source into a 1k/3k divider (answers around 1e-12 V, non-vacuous tolerances)
* bench.id: rob_cv_scale_1pv_divider_op
* bench.group: robustness
* bench.title: 1 pV source into a 1k/3k divider (answers around 1e-12 V, non-vacuous tolerances)
* bench.probes: v(1), v(2), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-24
* bench.floor: 1e-17
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, tiny_value, op
* bench.note: The default 1e-9 V absolute tolerance would make this case vacuous; tol.abs 1e-24 with tol.rel 1e-9 and floor 1e-17 make the 7.5e-13 V and 2.5e-16 A answers count. Regression anchor (linear solve has no absolute floor).
V1 1 0 1p
R1 1 2 1k
R2 2 0 3k
.op
.end
