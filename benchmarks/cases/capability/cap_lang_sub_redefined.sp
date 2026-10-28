Subcircuit defined twice
* bench.id: cap_lang_sub_redefined
* bench.group: capability
* bench.title: two .subckt DIV definitions (R1 = 1k, then 3k): ngspice keeps the first (4 V), RustSpice the last (2 V)
* bench.probes: v(out)
* bench.expect: either
* bench.tags: linear, op, subckt, redefinition
* bench.note: genuinely ambiguous, no verdict is possible: a duplicate .subckt is an authoring error with no defined precedence. ngspice-47 prints 'redefinition of .subckt div, ignored' and keeps the first definition (v(out) = 8 V * 1k/2k = 4 V), RustSpice silently lets the last one win (8 V * 1k/4k = 2 V); either behaviour is a convention and neither refuses; the outcome mismatch is still recorded in the capability matrix
.subckt DIV in out
R1 in out 1k
R2 out 0 1k
.ends DIV
.subckt DIV in out
R1 in out 3k
R2 out 0 1k
.ends DIV
V1 in 0 DC 8
X1 in out DIV
.op
.end
