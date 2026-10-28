Instance that names a model that does not exist
* bench.id: cap_lang_model_undefined
* bench.group: capability
* bench.title: D1 uses model NOSUCHMODEL which is never defined
* bench.probes: v(out)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: undefined model name is accepted and defaults silently (v(out) = 0.6929 V, exit 0, empty stderr; with NOSUCHMODEL D(IS=1e-12) defined RustSpice and ngspice give 0.5745 V); ngspice-47 refuses ("can't find model 'nosuchmodel'", exit 1)
* bench.tags: nonlinear, op, model, invalid_input, ngspice_error_ok
V1 in 0 DC 5
R1 in out 1k
D1 out 0 NOSUCHMODEL
.op
.end
