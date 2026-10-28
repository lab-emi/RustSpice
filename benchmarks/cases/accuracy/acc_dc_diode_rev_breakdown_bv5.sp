Diode with BV=5 V, IBV=1 mA reverse driven from -10 V through 1k
* bench.id: acc_dc_diode_rev_breakdown_bv5
* bench.group: accuracy
* bench.title: Diode with BV=5 V, IBV=1 mA reverse driven from -10 V through 1k
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.tags: nonlinear, op, diode, model_param, breakdown, silent_risk
* bench.note: Gap closed 2026-10-28: RustSpice now models diode reverse breakdown BV/IBV; the evidence below records the reference and the behaviour before the fix. tol.abs 1e-12 A; independent f64 root-find of the breakdown law v = -(BV + VT ln(I/IBV)) against the 1k load (CODATA constants at 300.15 K, gmin 1e-12) gives v(d) = -5.04141287 V, i = 4.958587 mA vs tight ngspice -5.04141286 V (1.4e-8 V)
V1 in 0 -10
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-14 N=1 BV=5 IBV=1m)
.op
.end
