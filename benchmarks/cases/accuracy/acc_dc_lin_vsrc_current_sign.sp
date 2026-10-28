Branch-current sign convention: supplying, absorbing and 0 V sources plus a current source
* bench.id: acc_dc_lin_vsrc_current_sign
* bench.group: accuracy
* bench.title: Branch-current sign convention: supplying, absorbing and 0 V sources plus a current source
* bench.probes: v(b), i(v1), i(v2), i(v3), i(vm)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.oracle: dc_linear
* bench.tags: linear, op, sign_convention
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 a 0 5
R1 a b 1k
V2 b 0 2
R2 b c 1k
V3 c 0 0
R3 a d 2k
Vm d 0 0
I1 0 c 1m
.op
.end
