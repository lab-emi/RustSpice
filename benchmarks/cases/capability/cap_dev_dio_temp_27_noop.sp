Diode with .temp 27 and tnom=27 (explicit defaults; ignoring them is correct)
* bench.id: cap_dev_dio_temp_27_noop
* bench.group: capability
* bench.title: Diode with .temp 27 and tnom=27 (explicit defaults; ignoring them is correct)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, temperature, control
V1 a 0 0
D1 a 0 DM

.model DM D(IS=1e-14)
.options tnom=27
.temp 27
.dc V1 0 0.7 0.05
.end
