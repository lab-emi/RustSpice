Divider 1 mohm / 1e12 ohm: 1e-12 A branch current read through a 0 V ammeter
* bench.id: acc_dc_lin_div_r_hi_gnd_leg
* bench.group: accuracy
* bench.title: Divider 1 mohm / 1e12 ohm: 1e-12 A branch current read through a 0 V ammeter
* bench.probes: v(a), v(b), i(vm)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.oracle: dc_linear
* bench.tags: linear, op, dynamic_range, wide_ratio
* bench.note: i(vm) = 1e-12 A so tol.abs is lowered to 1e-18 A; the current is read through an ammeter because i(v1) of this divider is a 1e-15 relative cancellation that no f64 nodal solver resolves (RS 2 %, ngspice 14 % off)
V1 in 0 1
R1 in a 1m
Vm a b 0
R2 b 0 1e12
.op
.end
