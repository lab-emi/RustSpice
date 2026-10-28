Unused BJT/JFET .model cards must not break a resistive circuit
* bench.id: cap_dev_model_unused_unsupported
* bench.group: capability
* bench.title: Unused BJT/JFET .model cards must not break a resistive circuit
* bench.probes: v(out), i(v1)
* bench.expect: match
* bench.tags: model_card, op, unused
V1 in 0 5
R1 in out 1k
R2 out 0 1k
.model QX NPN(IS=1e-16 BF=100)
.model JX NJF(VTO=-2 BETA=1m)
.op
.end
