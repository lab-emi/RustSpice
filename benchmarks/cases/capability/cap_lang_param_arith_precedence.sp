Operator precedence in a brace expression
* bench.id: cap_lang_param_arith_precedence
* bench.group: capability
* bench.title: {1k+2k*3} = 7k (multiplication first) and {10k/2/5} = 1k (division is left associative)
* bench.probes: v(a), v(b)
* bench.known_gap: expr-truncation
* bench.gap.note: {1k+2k*3} and {10k/2/5} are truncated to their first number (1k and 10k): v(a)=4 V, v(b)=7.27 V
* bench.tags: linear, op, param, expression
* bench.note: ngspice-47 (tight) agrees with an exact rational solve: {1k+2k*3} = 7k gives v(a) = 8 V * 1k/(7k+1k) = 1 V and {10k/2/5} = 1k gives v(b) = 8 V * 1k/(1k+1k) = 4 V. RustSpice keeps only the leading number of each expression (R1 = 1k, R4 = 10k) and answers v(a) = 4 V, v(b) = 7.27 V with exit 0 and no message.
V1 in 0 DC 8
R1 in a {1k+2k*3}
R2 a 0 1k
R3 in b 1k
R4 b 0 {10k/2/5}
.op
.end
