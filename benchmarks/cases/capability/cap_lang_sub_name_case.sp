Subcircuit names are case-insensitive
* bench.id: cap_lang_sub_name_case
* bench.group: capability
* bench.title: .subckt Div defined, instantiated as DIV and div
* bench.probes: v(o1), v(o2)
* bench.tags: linear, op, subckt, case
.subckt Div in out
R1 in out 1k
R2 out 0 1k
.ends Div
V1 in 0 DC 8
X1 in o1 DIV
X2 in o2 div
.op
.end
