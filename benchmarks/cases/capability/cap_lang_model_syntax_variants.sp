Syntax variants of the .model card
* bench.id: cap_lang_model_syntax_variants
* bench.group: capability
* bench.title: five diodes with the same parameters (IS=1e-12 N=1.5) written as 'D (..)', no parentheses, commas, spaced '=' and lower case; all five nodes must agree
* bench.probes: v(o1), v(o2), v(o3), v(o4), v(o5)
* bench.tags: nonlinear, op, model, syntax
V1 in 0 DC 5
R1 in o1 1k
R2 in o2 1k
R3 in o3 1k
R4 in o4 1k
R5 in o5 1k
D1 o1 0 M1
D2 o2 0 M2
D3 o3 0 M3
D4 o4 0 M4
D5 o5 0 M5
.model M1 D (IS=1e-12 N=1.5)
.model M2 D IS=1e-12 N=1.5
.model M3 D(IS=1e-12,N=1.5)
.model M4 D(IS = 1e-12 N = 1.5)
.model m5 d(is=1p n=1.5)
.op
.end
