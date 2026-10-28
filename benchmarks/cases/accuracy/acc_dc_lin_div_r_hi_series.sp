Divider 1e12 ohm / 1 mohm: output node at 1e-15 V
* bench.id: acc_dc_lin_div_r_hi_series
* bench.group: accuracy
* bench.title: Divider 1e12 ohm / 1 mohm: output node at 1e-15 V
* bench.probes: v(out), i(v1)
* bench.tol.abs: 1e-24
* bench.floor: 1e-18
* bench.oracle: dc_linear
* bench.tags: linear, op, dynamic_range, wide_ratio
* bench.note: v(out) = 1e-15 V and i(v1) = -1e-12 A: tol.abs lowered to 1e-24 so that neither probe is vacuous
V1 in 0 1
R1 in out 1e12
R2 out 0 1m
.op
.end
