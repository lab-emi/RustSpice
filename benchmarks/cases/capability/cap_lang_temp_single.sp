Single .temp card
* bench.id: cap_lang_temp_single
* bench.group: capability
* bench.title: .temp 85 moves a forward-biased diode by roughly -0.1 V relative to 27 C
* bench.probes: v(out)
* bench.tags: nonlinear, op, temperature
* bench.note: Independent closed form: SPICE3 diode temperature law IS(T) = IS*exp((T/Tn-1)*EG/Vt(T))*(T/Tn)^XTI (EG = 1.11 eV, XTI = 3, N = 1, Vt = kT/q at T) plus gmin 1e-12 S, solved with brentq for the divider 5 V - 1k - diode at T = 358.15 K, Tn = 300.15 K gives v(out) = 0.596611 V = tight ngspice. RustSpice prints the 27 C value 0.692888 V and gives the same number for .temp -250 and 100, i.e. the card is dropped. [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-14)
.temp 85
.op
.end
