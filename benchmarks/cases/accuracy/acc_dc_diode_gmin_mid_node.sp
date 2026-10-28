Two reverse-biased diodes in series (IS=5e-13 and 5e-14) from 5 V: the middle node is set by leakage balance and gmin
* bench.id: acc_dc_diode_gmin_mid_node
* bench.group: accuracy
* bench.title: Two reverse-biased diodes in series (IS=5e-13 and 5e-14) from 5 V: the middle node is set by leakage balance and gmin
* bench.probes: v(m), i(v1)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, gmin, reverse
* bench.note: v(m) = 2.5 + (IS1 - IS2) / (2 gmin) = 2.725 V analytically; i(v1) ~ 4e-13 A so tol.abs is 1e-18 A RustSpice and ngspice agree to 1e-28 here; the dc_nonlinear oracle differs from BOTH by 4.65e-7 V on v(m) (a model detail of the deep-reverse branch, 1.7e-7 relative), so the oracle columns are informational only for this deck and the closed form 2.725 V is the first-order value.
V1 in 0 5
D1 m in da
D2 0 m db
.model da D(IS=5e-13 N=1)
.model db D(IS=5e-14 N=1)
.op
.end
