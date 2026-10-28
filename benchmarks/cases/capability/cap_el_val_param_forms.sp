Values from .param in R, V, DC V, I and G cards: {rv}, r={rv}, r=2k, quoted
* bench.id: cap_el_val_param_forms
* bench.group: capability
* bench.title: Values from .param in R, V, DC V, I and G cards: {rv}, r={rv}, r=2k, quoted
* bench.probes: i(v1), i(v2), i(v3), i(v4), v(in), v(o1), v(o2), v(o3)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-12
* bench.expect: match
* bench.tags: parser, passive, source, op, value_syntax, param
* bench.note: The four resistor forms must give R = 2k so each branch current is -5e-4 A (parameter names are case-insensitive); v(in)=5, v(o1)=gm*5*2k=20, v(o2)=1m*2k=2, v(o3)=5.
.PARAM RV=2k vdd=5 gm=2m ib=1m
V1 a 0 1
R1 a 0 {rv}
V2 c 0 1
R2 c 0 r=2k
V3 d 0 1
R3 d 0 r={rv}
V4 e 0 1
R4 e 0 'rv'
V5 in 0 {vdd}
G1 0 o1 in 0 {gm}
R5 o1 0 {rv}
I1 0 o2 {ib}
R6 o2 0 {rv}
V6 o3 0 DC {vdd}
R7 o3 0 {rv}
.op
.end
