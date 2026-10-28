Diode reverse-biased by 1 kV through 1k (no breakdown model: i = -IS)
* bench.id: rob_cv_diode_reverse_1kv_op
* bench.group: robustness
* bench.title: Diode reverse-biased by 1 kV through 1k (no breakdown model: i = -IS)
* bench.probes: v(2), i(v1)
* bench.tol.rel: 1e-12
* bench.tol.abs: 1e-15
* bench.floor: 1e-15
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, reverse, op
* bench.note: Regression anchor: reverse leakage of 1e-9 A (gmin*1000 V + IS) is reproduced exactly (RustSpice vs ngspice-tight: 0 V on v(2), 8e-18 A on i(v1)). Tightened (calibration): with the defaults the leakage was invisible (i(v1)=1e-9 A is below the 1e-6 floor, hence vacuous, and v(2)=-999.999999 had a 1e-3 V tolerance); tol.rel 1e-12 / tol.abs 1e-15 / floor 1e-15 make both probes real checks (a zero-leakage diode would be off by 1e-9 A and 1e-6 V).
V1 1 0 -1000
R1 1 2 1k
D1 2 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
