DC sweep with zero step
* bench.id: cap_lang_dc_zero_step
* bench.group: capability
* bench.title: .dc V1 0 1 0 has an infinite number of points
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, dc, invalid_input, ngspice_error_ok
V1 in 0 1
R1 in out 1k
R2 out 0 1k
.dc V1 0 1 0
.end
