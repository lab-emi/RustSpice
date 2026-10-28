Diode I-V with .options tnom=50 (simulated at default 27 C)
* bench.id: cap_dev_dio_tnom_option
* bench.group: capability
* bench.title: Diode I-V with .options tnom=50 (simulated at default 27 C)
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.known_gap: silent-ignore-temperature
* bench.gap.note: .options tnom=50 ignored: ngspice rescales IS from 50 C to 27 C
* bench.tags: diode, dc, temperature
* bench.note: evidence: RustSpice output is bit-identical with and without .options tnom=50; textbook IS rescaling from 50 C to 27 C equals tight ngspice to 1e-5 relative (0.214 mA vs RustSpice 5.67 mA at 0.7 V, a factor 26)
V1 a 0 0
D1 a 0 DM

.model DM D(IS=1e-14)
.options tnom=50
.dc V1 0 0.7 0.05
.end
