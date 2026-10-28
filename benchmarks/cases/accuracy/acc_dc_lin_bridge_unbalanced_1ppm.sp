Wheatstone bridge unbalanced by 1 ppm: the 2.4e-9 A detector current comes from a 1.2e-7 V difference of two 5 V nodes
* bench.id: acc_dc_lin_bridge_unbalanced_1ppm
* bench.group: accuracy
* bench.title: Wheatstone bridge unbalanced by 1 ppm: the 2.4e-9 A detector current comes from a 1.2e-7 V difference of two 5 V nodes
* bench.probes: v(a), v(b), i(vdet)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.oracle: dc_linear
* bench.tags: linear, op, cancellation, bridge
* bench.note: tol.abs lowered to 1e-18 A for the detector current; default tol.rel kept (RS agrees to 5e-9 relative)
V1 t 0 10
R1 t a 1k
R2 a 0 1k
R3 t b 1k
R4 b 0 1.000001k
Rg a c 50
Vdet c b 0
.op
.end
