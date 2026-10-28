Diode model declared with a misspelt type DD (ngspice rejects it)
* bench.id: cap_dev_model_type_typo
* bench.group: capability
* bench.title: Diode model declared with a misspelt type DD (ngspice rejects it)
* bench.probes: i(v1)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: misspelt model type DD silently ignored (exit 0, empty stderr): the diode runs with the default model, i(v1) at 0.6 V = -1.187e-4 A instead of -1.187e-2 A with IS=1e-12 (type corrected to D: RustSpice = ngspice); ngspice-47 aborts ('model type mismatch', exit 1)
* bench.tags: model_card, diode, dc, ngspice_error_ok
V1 a 0 0
D1 a 0 DM
.model DM DD(IS=1e-12)
.dc V1 0 0.6 0.05
.end
