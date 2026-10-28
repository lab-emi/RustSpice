Diode model declared with a misspelt type DD (ngspice rejects it)
* bench.id: cap_dev_model_type_typo
* bench.group: capability
* bench.title: Diode model declared with a misspelt type DD (ngspice rejects it)
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.tags: model_card, diode, dc, ngspice_error_ok
V1 a 0 0
D1 a 0 DM
.model DM DD(IS=1e-12)
.dc V1 0 0.6 0.05
.end
