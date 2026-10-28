DC sweep whose step does not divide the span
* bench.id: cap_lang_dc_step_not_dividing
* bench.group: capability
* bench.title: .dc V1 0 1 0.3 ends at 0.9 (stop is not reached), 4 points
* bench.probes: v(out), i(v1)
* bench.tags: linear, dc
V1 in 0 0
R1 in out 1k
R2 out 0 1k
.dc V1 0 1 0.3
.end
