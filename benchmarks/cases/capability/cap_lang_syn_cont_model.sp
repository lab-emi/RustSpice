Continuation line inside a .model card
* bench.id: cap_lang_syn_cont_model
* bench.group: capability
* bench.title: .model DM D(IS=1e-12 / + N=1.5) split inside the parentheses
* bench.probes: v(out)
* bench.tags: nonlinear, op, syntax, continuation, model
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-12
+ N=1.5)
.op
.end
