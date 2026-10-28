Resistor value suffixes k K meg MEG Meg g G t T (every branch carries 1 mA)
* bench.id: cap_el_val_suffix_big
* bench.group: capability
* bench.title: Resistor value suffixes k K meg MEG Meg g G t T (every branch carries 1 mA)
* bench.probes: i(v1), i(v2), i(v3), i(v4), i(v5), i(v6), i(v7), i(v8), i(v9), i(v10), i(v11)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: parser, passive, op, value_syntax
* bench.note: One V source across one R per node, V = R*1mA, so every probe must be -1e-3 A; a wrong suffix shows up on exactly one probe. Tolerances tightened because the circuit is exact.
V1 n1 0 1
R1 n1 0 1k
V2 n2 0 1
R2 n2 0 1K
V3 n3 0 1e3
R3 n3 0 1meg
V4 n4 0 1e3
R4 n4 0 1MEG
V5 n5 0 1e3
R5 n5 0 1Meg
V6 n6 0 1e6
R6 n6 0 1g
V7 n7 0 1e6
R7 n7 0 1G
V8 n8 0 1e9
R8 n8 0 1t
V9 n9 0 1e9
R9 n9 0 1T
V10 n10 0 4.7e3
R10 n10 0 4.7Meg
V11 n11 0 2.2
R11 n11 0 2.2k
.op
.end
