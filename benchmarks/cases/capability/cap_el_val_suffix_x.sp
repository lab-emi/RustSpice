Value suffix x: meg in RustSpice (HSPICE style), an ignored letter in ngspice
* bench.id: cap_el_val_suffix_x
* bench.group: capability
* bench.title: Value suffix x: meg in RustSpice (HSPICE style), an ignored letter in ngspice
* bench.probes: i(v1), i(v2)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-12
* bench.expect: either
* bench.tags: parser, passive, op, value_syntax, dialect
* bench.note: Neither tool can be judged: the letter x after a number is a dialect choice with no independent oracle. ngspice-47 gives i(v1) = i(v2) = -1.000e-3 A for V = 1e-3 and 4.7e-3 across R = 1x and 4.7x, so R is 1 and 4.7 ohm (x is an ignored letter, like z, y and other unknown letters). RustSpice reads x as meg on purpose (an explicit branch of its number parser, an HSPICE-style scale factor) and gives i(v1) = -1e-9 A (R = 1e6 and 4.7e6 ohm). A netlist written for HSPICE would be misread by ngspice the other way round. Not verified: other simulators (none installed), so the HSPICE attribution is from memory; expect either records the divergence without calling either side wrong.
V1 n1 0 1e-3
R1 n1 0 1x
V2 n2 0 4.7e-3
R2 n2 0 4.7x
.op
.end
