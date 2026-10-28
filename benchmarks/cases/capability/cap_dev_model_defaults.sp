Diode .model with no parameters (all defaults)
* bench.id: cap_dev_model_defaults
* bench.group: capability
* bench.title: Diode .model with no parameters (all defaults)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: model_card, diode, dc
V1 a 0 0
D1 a 0 DM
.model DM D
.dc V1 0 0.6 0.05
.end
