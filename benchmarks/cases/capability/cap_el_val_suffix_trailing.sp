Characters after the scale suffix are ignored (1kOhm, 1k5, 1e3k, 100ohm)
* bench.id: cap_el_val_suffix_trailing
* bench.group: capability
* bench.title: Characters after the scale suffix are ignored (1kOhm, 1k5, 1e3k, 100ohm)
* bench.probes: i(v1), i(v2), i(v3), i(v4), i(v5), i(v6), i(v7), i(v8), i(v9), i(v10), i(v11), i(v12)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-12
* bench.expect: match
* bench.tags: parser, passive, op, value_syntax, trap
* bench.note: SPICE reads the leading number plus the first suffix letters and ignores the rest: 1k5 is 1k (not 1.5k), 1e3k is 1e6, 100ohm is 100 (o is no suffix), 1mm is 1m. V = R*1mA so each probe is -1e-3 A.
V1 n1 0 1
R1 n1 0 1kOhm
V2 n2 0 10
R2 n2 0 10kohm
V3 n3 0 4.7
R3 n3 0 4.7KOHM
V4 n4 0 1e3
R4 n4 0 1megohm
V5 n5 0 0.1
R5 n5 0 100ohm
V6 n6 0 1
R6 n6 0 1k5
V7 n7 0 5e-9
R7 n7 0 5u5
V8 n8 0 1e3
R8 n8 0 1Meg5
V9 n9 0 1e3
R9 n9 0 1e3k
V10 n10 0 1e-3
R10 n10 0 1e-3k
V11 n11 0 1e-6
R11 n11 0 1mm
V12 n12 0 1
R12 n12 0 1kV
.op
.end
