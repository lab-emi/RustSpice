Upper, lower and mixed case directives
* bench.id: cap_lang_syn_case_directives
* bench.group: capability
* bench.title: .Model / .OP / .End and DC / Dc keywords in mixed case
* bench.probes: v(out)
* bench.tags: nonlinear, op, syntax, case
V1 in 0 Dc 5
R1 in out 1K
D1 out 0 dm
.MODEL DM d(Is=1E-12 n=1.5)
.Op
.End
