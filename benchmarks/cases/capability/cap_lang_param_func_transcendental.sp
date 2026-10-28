Transcendental functions in expressions
* bench.id: cap_lang_param_func_transcendental
* bench.group: capability
* bench.title: exp, ln and log10 inside brace expressions
* bench.probes: v(a), v(b)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: Invalid resistance value `{exp(0)*1k}`
* bench.tags: linear, op, param, functions
* bench.note: ngspice-47 computes v(a) = 10 V * 2k/(1k+2k) = 6.667 V and v(b) = 10 V * 2k/(3k+2k) = 4 V (hand-checked: exp(0)=1, ln(exp(2))=2, log10(1000)=3, log10(100)=2)
V1 in 0 DC 10
R1 in a {exp(0)*1k}
R2 a 0 {ln(exp(2))*1k}
R3 in b {log10(1000)*1k}
R4 b 0 {log10(100)*1k}
.op
.end
