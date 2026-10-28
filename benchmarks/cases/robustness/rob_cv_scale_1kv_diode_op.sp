1 kV source through 10k into a diode (hundred-volt-scale rails)
* bench.id: rob_cv_scale_1kv_diode_op
* bench.group: robustness
* bench.title: 1 kV source through 10k into a diode (hundred-volt-scale rails)
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, large_value, op
* bench.note: Regression anchor: the diode stays at 0.7742 V with 1 kV on the rail.
V1 1 0 1000
R1 1 2 10k
D1 2 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
