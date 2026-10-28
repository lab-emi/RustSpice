Diode IS=1e-14 reverse biased by 5 V through 1k: the 5e-12 A leakage is gmin * V
* bench.id: acc_dc_diode_rev_5v_gmin_leakage
* bench.group: accuracy
* bench.title: Diode IS=1e-14 reverse biased by 5 V through 1k: the 5e-12 A leakage is gmin * V
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-17
* bench.floor: 1e-14
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, reverse, gmin
* bench.note: i(v1) ~ 5e-12 A: tol.abs lowered to 1e-17 A (V/R rounding noise floor is 5e-19 A)
V1 in 0 -5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
