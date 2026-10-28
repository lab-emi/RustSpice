Diode saturation-current temperature exponent XTI=1.5 at .temp 85
* bench.id: cap_dev_dio_xti
* bench.group: capability
* bench.title: Diode saturation-current temperature exponent XTI=1.5 at .temp 85
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, temperature, model_param
* bench.note: slug: the whole mismatch comes from the ignored .temp; deleting only the .temp card (XTI=1.5 kept) makes tight ngspice equal RustSpice to 1e-14 A, and RustSpice is bit-identical for .temp 85, .options temp=85 and with or without XTI; XTI acts only through T != TNOM, so this deck cannot isolate XTI: after .temp is honoured the deck still needs XTI in the IS(T) law (XTI=1.5 gives 96.3 mA, the default XTI gives 125.5 mA in ngspice at 0.7 V, 85 C); textbook IS(T) scaling at 85 C with XTI=1.5 equals tight ngspice to 1e-5 relative [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 a 0 0
D1 a 0 DM

.model DM D(IS=1e-14 XTI=1.5)
.temp 85
.dc V1 0 0.7 0.05
.end
