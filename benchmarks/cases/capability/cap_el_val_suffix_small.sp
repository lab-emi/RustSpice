Resistor value suffixes m M u U n N mil (M is milli, mil is 25.4 um)
* bench.id: cap_el_val_suffix_small
* bench.group: capability
* bench.title: Resistor value suffixes m M u U n N mil (M is milli, mil is 25.4 um)
* bench.probes: i(v1), i(v2), i(v3), i(v4), i(v5), i(v6), i(v7), i(v8), i(v9)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: parser, passive, op, value_syntax
* bench.note: V = R*1mA again so every probe is -1e-3 A. Smallest R is 1 nohm, above the 1e-12 ohm floor ngspice applies.
V1 n1 0 1e-6
R1 n1 0 1m
V2 n2 0 1e-6
R2 n2 0 1M
V3 n3 0 1e-9
R3 n3 0 1u
V4 n4 0 1e-9
R4 n4 0 1U
V5 n5 0 1e-12
R5 n5 0 1n
V6 n6 0 1e-12
R6 n6 0 1N
V7 n7 0 2.54e-8
R7 n7 0 1mil
V8 n8 0 6.35e-8
R8 n8 0 2.5mil
V9 n9 0 1.5e-6
R9 n9 0 1.5m
.op
.end
