Diode reverse bias -5 V leakage current (IS=1e-12)
* bench.id: cap_dev_dio_reverse_leak
* bench.group: capability
* bench.title: Diode reverse bias -5 V leakage current (IS=1e-12)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.floor: 1e-15
* bench.expect: match
* bench.tags: diode, op, reverse, leakage
* bench.note: leakage-level probe: floor 1e-15 A (the 6.0e-12 A is IS + gmin*5 V, so the default 1e-6 floor would make it vacuous); IS raised from 1e-14 to 1e-12 so that IS (1e-12) is a sixth of the leakage instead of 0.2% (a 5% change of IS was below tol.abs before, now 50x above it); closed form i=IS*(1-exp(-5/VT))+gmin*5 = 6.0e-12 A equals both simulators
V1 a 0 -5
D1 a 0 DM
.model DM D(IS=1e-12)
.op
.end
