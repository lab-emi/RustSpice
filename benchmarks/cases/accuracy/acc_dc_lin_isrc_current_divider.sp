Current divider: 1 mA into four parallel branches read through 0 V ammeters
* bench.id: acc_dc_lin_isrc_current_divider
* bench.group: accuracy
* bench.title: Current divider: 1 mA into four parallel branches read through 0 V ammeters
* bench.probes: v(n), i(vm1), i(vm2), i(vm3), i(vm4)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.oracle: dc_linear
* bench.tags: linear, op, isrc, sign_convention
* bench.note: tol.rel 1e-9 / tol.abs 1e-15 (branch currents are 1e-4 A scale)
I1 0 n 1m
Vm1 n a1 0
R1 a1 0 1k
Vm2 n a2 0
R2 a2 0 2.2k
Vm3 n a3 0
R3 a3 0 4.7k
Vm4 n a4 0
R4 a4 0 10k
.op
.end
