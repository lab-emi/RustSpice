1 Pohm / 1 Pohm divider on 1 V (conductances of 1e-15 S)
* bench.id: rob_cv_res_1e15_divider_op
* bench.group: robustness
* bench.title: 1 Pohm / 1 Pohm divider on 1 V (conductances of 1e-15 S)
* bench.probes: v(2), i(v1)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-20
* bench.floor: 1e-18
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, large_resistance, op
* bench.note: Regression anchor for resistances far above the 1e12 ohm of gmin; exact 0.5 V and 5e-16 A in both (tol.abs 1e-20 and floor 1e-18 keep the femto-amp branch current non-vacuous).
V1 1 0 1
R1 1 2 1e15
R2 2 0 1e15
.op
.end
