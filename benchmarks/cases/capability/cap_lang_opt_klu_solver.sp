Solver selection option
* bench.id: cap_lang_opt_klu_solver
* bench.group: capability
* bench.title: .options klu (ngspice linear solver switch) must be tolerated
* bench.probes: v(out)
* bench.tags: linear, op, options, syntax
.options klu
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
