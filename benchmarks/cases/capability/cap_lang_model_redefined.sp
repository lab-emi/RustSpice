Model defined twice
* bench.id: cap_lang_model_redefined
* bench.group: capability
* bench.title: .model DM D(IS=1e-12 N=1.5) then .model DM D(IS=1e-14 N=1): ngspice keeps the first definition (0.859 V), RustSpice the last (0.693 V)
* bench.probes: v(out)
* bench.expect: either
* bench.tags: nonlinear, op, model, redefinition
* bench.note: genuinely ambiguous, no verdict is possible: a duplicate .model is an authoring error with no defined precedence. ngspice-47 keeps the first definition (v(out) = 0.8591 V = the IS=1e-12 N=1.5 value, independently reproduced in mpmath), RustSpice silently lets the last one win (0.6929 V); either behaviour is a convention, the only objective defect is that neither refuses and RustSpice says nothing, which the suite cannot observe (outcome mismatch is still recorded in the matrix)
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-12 N=1.5)
.model DM D(IS=1e-14 N=1)
.op
.end
