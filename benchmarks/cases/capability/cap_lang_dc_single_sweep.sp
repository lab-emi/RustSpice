Single source DC sweep
* bench.id: cap_lang_dc_single_sweep
* bench.group: capability
* bench.title: .dc V1 0 5 1 on a resistor divider, 6 points
* bench.probes: v(out), i(v1)
* bench.oracle: dc_nonlinear
* bench.tags: linear, dc, anchor
V1 in 0 0
R1 in out 1k
R2 out 0 3k
.dc V1 0 5 1
.end
