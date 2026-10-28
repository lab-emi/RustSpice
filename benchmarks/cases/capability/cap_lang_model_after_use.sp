Model card after the element that uses it
* bench.id: cap_lang_model_after_use
* bench.group: capability
* bench.title: D1 appears before .model DM; card order must not matter
* bench.probes: v(out)
* bench.tags: nonlinear, op, model, syntax
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.op
.model DM D(IS=1e-12 N=1.5)
.end
