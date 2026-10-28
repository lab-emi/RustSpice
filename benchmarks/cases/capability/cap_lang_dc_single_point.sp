DC sweep with start equal to stop
* bench.id: cap_lang_dc_single_point
* bench.group: capability
* bench.title: .dc V1 2 2 1 yields exactly one point
* bench.probes: v(out), i(v1)
* bench.tags: linear, dc
V1 in 0 0
R1 in out 1k
R2 out 0 1k
.dc V1 2 2 1
.end
