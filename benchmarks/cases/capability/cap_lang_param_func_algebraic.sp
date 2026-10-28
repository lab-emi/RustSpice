Algebraic functions in expressions
* bench.id: cap_lang_param_func_algebraic
* bench.group: capability
* bench.title: sqrt, pow, abs, min and max inside brace expressions
* bench.probes: v(a), v(b), v(c)
* bench.tags: linear, op, param, functions
* bench.note: ngspice-47 computes v(a) = 10 V * 8k/(4k+8k) = 6.667 V, v(b) = 10 V * 5k/(2k+5k) = 7.143 V, v(c) = 10 V * 1k/(3k+1k) = 2.5 V (hand-checked: sqrt(16)=4, pow(2,3)=8, min=2k, max=5k, abs=3k)
V1 in 0 DC 10
R1 in a {sqrt(16)*1k}
R2 a 0 {pow(2,3)*1k}
R3 in b {min(2k,5k)}
R4 b 0 {max(2k,5k)}
R5 in c {abs(-3k)}
R6 c 0 1k
.op
.end
