Parameter used as a model parameter value
* bench.id: cap_lang_param_in_model
* bench.group: capability
* bench.title: .model DM D(IS={isat} N=1.5) takes IS from .param isat=1e-12
* bench.probes: v(out)
* bench.tags: nonlinear, op, param, model
.param isat=1e-12
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS={isat} N=1.5)
.op
.end
