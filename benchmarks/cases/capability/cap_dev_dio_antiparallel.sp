Anti-parallel diode limiter, +-3 V sweep through 1 kohm
* bench.id: cap_dev_dio_antiparallel
* bench.group: capability
* bench.title: Anti-parallel diode limiter, +-3 V sweep through 1 kohm
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, dc, textbook
V1 in 0 0
R1 in a 1k
D1 a 0 DM
D2 0 a DM
.model DM D(IS=1e-14)
.dc V1 -3 3 0.25
.end
