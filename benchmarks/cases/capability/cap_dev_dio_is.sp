Diode IS=1e-12 forward I-V
* bench.id: cap_dev_dio_is
* bench.group: capability
* bench.title: Diode IS=1e-12 forward I-V
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-12)
.dc V1 0 0.6 0.025
.end
