DC sweep downward with a negative step
* bench.id: cap_lang_dc_negative_step
* bench.group: capability
* bench.title: .dc V1 5 0 -1 sweeps downward, 6 points in descending order
* bench.probes: v(out), i(v1)
* bench.tags: linear, dc
V1 in 0 0
R1 in out 1k
R2 out 0 3k
.dc V1 5 0 -1
.end
