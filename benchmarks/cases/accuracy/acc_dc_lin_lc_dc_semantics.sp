DC operating point: inductors are shorts and capacitors are open circuits
* bench.id: acc_dc_lin_lc_dc_semantics
* bench.group: accuracy
* bench.title: DC operating point: inductors are shorts and capacitors are open circuits
* bench.probes: v(a), v(b), i(v1), i(l1), i(l2)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.tags: linear, op, lc
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13); no dc_linear oracle (L/C elements); closed form: i = 5/3k, v(b) = 5 - 1k*i
V1 in 0 5
L1 in a 1m
R1 a b 1k
C1 b 0 1u
R2 b c 2k
L2 c 0 10u
.op
.end
