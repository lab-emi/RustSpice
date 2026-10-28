Diode IS=1e-18 from 0..5 V through 1 kohm: DC sweep (control for the OP deck)
* bench.id: cap_dev_dio_is_small_sweep
* bench.group: capability
* bench.title: Diode IS=1e-18 from 0..5 V through 1 kohm: DC sweep (control for the OP deck)
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, dc, small_is, control
V1 in 0 0
R1 in a 1k
D1 a 0 DM
.model DM D(IS=1e-18)
.dc V1 0 5 0.25
.end
