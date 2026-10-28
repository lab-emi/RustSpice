Diode .model with IS given twice (1e-12 then 1e-10)
* bench.id: cap_dev_model_dup_param
* bench.group: capability
* bench.title: Diode .model with IS given twice (1e-12 then 1e-10)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: model_card, diode, dc
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-12 IS=1e-10)
.dc V1 0 0.6 0.05
.end
