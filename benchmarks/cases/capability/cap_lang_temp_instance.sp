Instance temperature
* bench.id: cap_lang_temp_instance
* bench.group: capability
* bench.title: D1 ... temp=85 sets the temperature of one diode only
* bench.probes: v(out)
* bench.tags: nonlinear, op, temperature
* bench.note: Independent closed form: SPICE3 diode temperature law IS(T) = IS*exp((T/Tn-1)*EG/Vt(T))*(T/Tn)^XTI (EG = 1.11 eV, XTI = 3, N = 1, Vt = kT/q at T) plus gmin 1e-12 S, solved with brentq for the divider 5 V - 1k - diode at T = 358.15 K, Tn = 300.15 K gives v(out) = 0.596611 V = tight ngspice, which honours the instance temp= exactly like .temp 85. RustSpice prints the 27 C value 0.692888 V. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM temp=85
.model DM D(IS=1e-14)
.op
.end
