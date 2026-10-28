Diode .model with an unknown parameter FOO=3 (ngspice warns and ignores)
* bench.id: cap_dev_model_unknown_param
* bench.group: capability
* bench.title: Diode .model with an unknown parameter FOO=3 (ngspice warns and ignores)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: model_card, diode, dc
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-12 FOO=3)
.dc V1 0 0.6 0.05
.end
