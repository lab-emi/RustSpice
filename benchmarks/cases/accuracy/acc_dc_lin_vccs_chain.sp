Three VCCS stages with floating control pairs, loads and a bridging resistor
* bench.id: acc_dc_lin_vccs_chain
* bench.group: accuracy
* bench.title: Three VCCS stages with floating control pairs, loads and a bridging resistor
* bench.probes: v(n1), v(n2), v(n3)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.oracle: dc_linear
* bench.tags: linear, op, vccs
* bench.note: tol.rel/tol.abs tightened (f64 LU solves agree with tight ngspice to ~1e-13)
V1 in 0 0.5
V2 ref 0 -0.2
G1 0 n1 in ref 2m
R1 n1 0 3k
G2 n2 0 n1 0 1.5m
R2 n2 0 2k
G3 n3 n2 n2 n1 4m
R3 n3 0 5k
R4 n3 n1 20k
.op
.end
