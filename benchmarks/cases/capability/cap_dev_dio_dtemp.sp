Diode instance dtemp=58 (85 C effective)
* bench.id: cap_dev_dio_dtemp
* bench.group: capability
* bench.title: Diode instance dtemp=58 (85 C effective)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.known_gap: silent-ignore-temperature
* bench.gap.note: instance dtemp ignored (fixed 300.15 K)
* bench.tags: diode, dc, temperature, instance_param
* bench.note: evidence: RustSpice output is bit-identical with and without dtemp; textbook IS(T) scaling at 85 C (27 C + 58 K) equals tight ngspice to 1e-5 relative (125 mA vs RustSpice 5.67 mA at 0.7 V)
V1 a 0 0
D1 a 0 DM dtemp=58
.model DM D(IS=1e-14)
.dc V1 0 0.7 0.05
.end
