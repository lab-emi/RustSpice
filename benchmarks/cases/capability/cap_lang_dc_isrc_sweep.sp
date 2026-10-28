DC sweep of a current source
* bench.id: cap_lang_dc_isrc_sweep
* bench.group: capability
* bench.title: .dc I1 0 3m 1m sweeps a current source into a resistor
* bench.probes: v(n1)
* bench.tags: linear, dc
I1 0 n1 0
R1 n1 0 1k
.dc I1 0 3m 1m
.end
