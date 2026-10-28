Diode instance multiplier m=3
* bench.id: cap_dev_dio_m
* bench.group: capability
* bench.title: Diode instance multiplier m=3
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.known_gap: silent-ignore-multiplier
* bench.gap.note: m=3 ignored: 5.67 mA at 0.7 V (ngspice 17.0 mA)
* bench.tags: diode, dc, instance_param, multiplier
* bench.note: evidence: RustSpice output is bit-identical with and without m=3; tight ngspice is exactly 3.0x. Independent closed form 3*IS*(exp(V/Vt)-1), Vt = kT/q at 300.15 K, gives -17.011 mA at 0.7 V (tight ngspice -17.011 mA, 9e-6 relative; RustSpice -5.670 mA). Only m is dropped: RustSpice with area=3 or IS=3e-14 gives exactly the ngspice m=3 value.
V1 a 0 0
D1 a 0 DM m=3
.model DM D(IS=1e-14)
.dc V1 0 0.7 0.05
.end
