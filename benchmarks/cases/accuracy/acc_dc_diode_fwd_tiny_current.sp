Diode IS=1e-14 at 0.2 V through 1k: 2.3e-11 A forward current, gmin leakage is 1 % of it
* bench.id: acc_dc_diode_fwd_tiny_current
* bench.group: accuracy
* bench.title: Diode IS=1e-14 at 0.2 V through 1k: 2.3e-11 A forward current, gmin leakage is 1 % of it
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, dynamic_range, gmin
* bench.note: tol.abs lowered to 1e-18 A because i(v1) is 2e-11 A
V1 in 0 0.2
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
