Numeric literal forms: .5k 5.k +1k 1.0E3 1e+3 1E-0 007 2e0k
* bench.id: cap_el_val_number_forms
* bench.group: capability
* bench.title: Numeric literal forms: leading dot, trailing dot, sign, exponent spellings
* bench.probes: i(v1), i(v2), i(v3), i(v4), i(v5), i(v6), i(v7), i(v8)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: parser, passive, op, value_syntax
* bench.note: Every branch is V = R*1mA so each probe must be -1e-3 A: R1 = .5k (500), R2 = 5.k (5000), R3 = +1k, R4 = 1.0E3, R5 = 1e+3, R6 = 1E-0 (1 ohm), R7 = 007 (7 ohm), R8 = 2e0k (2000). Both simulators read all eight the same way (checked with ngspice-47 and the dc_linear oracle).
V1 n1 0 0.5
R1 n1 0 .5k
V2 n2 0 5
R2 n2 0 5.k
V3 n3 0 1
R3 n3 0 +1k
V4 n4 0 1000
R4 n4 0 1.0E3
V5 n5 0 1000
R5 n5 0 1e+3
V6 n6 0 1e-3
R6 n6 0 1E-0
V7 n7 0 7e-3
R7 n7 0 007
V8 n8 0 2
R8 n8 0 2e0k
.op
.end
