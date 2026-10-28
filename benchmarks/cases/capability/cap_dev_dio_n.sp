Diode emission coefficient N=1.8 forward I-V
* bench.id: cap_dev_dio_n
* bench.group: capability
* bench.title: Diode emission coefficient N=1.8 forward I-V
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-14 N=1.8)
.dc V1 0 1.2 0.05
.end
