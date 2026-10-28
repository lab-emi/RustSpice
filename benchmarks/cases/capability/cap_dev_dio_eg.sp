Diode bandgap EG=0.8 eV at .temp 85
* bench.id: cap_dev_dio_eg
* bench.group: capability
* bench.title: Diode bandgap EG=0.8 eV at .temp 85
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, temperature, model_param
* bench.note: slug: the whole mismatch comes from the ignored .temp; deleting only the .temp card (EG=0.8 kept) makes tight ngspice equal RustSpice to 1e-14 A, and RustSpice is bit-identical for .temp 85, .options temp=85 and with or without EG; EG acts only through T != TNOM, so this deck cannot isolate EG: after .temp is honoured the deck still needs EG in the IS(T) law (EG=0.8 gives 18.0 mA, the default EG gives 125.5 mA in ngspice at 0.7 V, 85 C); textbook IS(T) scaling at 85 C with EG=0.8 equals tight ngspice to 1e-5 relative [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0
D1 a 0 DM

.model DM D(IS=1e-14 EG=0.8)
.temp 85
.dc V1 0 0.7 0.05
.end
