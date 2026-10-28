Diode instance AREA=2 scales saturation current
* bench.id: cap_dev_dio_area
* bench.group: capability
* bench.title: Diode instance AREA=2 scales saturation current
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, instance_param
V1 a 0 0
D1 a 0 DM area=2
.model DM D(IS=1e-14)
.dc V1 0 0.7 0.05
.end
