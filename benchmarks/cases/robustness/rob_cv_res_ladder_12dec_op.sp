Ladder mixing 1 mohm ... 1e12 ohm resistors (12 decades of conductance, linear)
* bench.id: rob_cv_res_ladder_12dec_op
* bench.group: robustness
* bench.title: Ladder mixing 1 mohm ... 1e12 ohm resistors (12 decades of conductance, linear)
* bench.probes: v(n1), v(n2), v(n3), v(n4), i(v1)
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.floor: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: linear, ill_conditioned, op
* bench.note: Regression anchor: matrix condition number ~1e15. Against a 60-digit exact solution the errors are RustSpice 2e-11, ngspice-47 3e-11 and dense f64 LU 8.6e-11 V (v(n2)); tol.rel 1e-8 covers the roundoff all three share. Calibration evidence: i(v1) is about 1e-6 A, exactly at the default 1e-6 floor, so it was silently excluded as vacuous; floor 1e-9 puts it back under test (RustSpice error 7.6e-14 A against a 1e-12 A allowance).
V1 n0 0 1
R1 n0 n1 1e-3
R2 n1 n2 1e3
R3 n2 n3 1e-3
R4 n3 n4 1e9
R5 n4 0 1e9
R6 n3 0 1e6
R7 n1 0 1e12
.op
.end
