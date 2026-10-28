Instance that names a model that does not exist
* bench.id: cap_lang_model_undefined
* bench.group: capability
* bench.title: D1 uses model NOSUCHMODEL which is never defined
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: nonlinear, op, model, invalid_input, ngspice_error_ok
V1 in 0 DC 5
R1 in out 1k
D1 out 0 NOSUCHMODEL
.op
.end
