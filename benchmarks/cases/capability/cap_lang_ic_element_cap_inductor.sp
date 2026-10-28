Element ic on a capacitor and on an inductor
* bench.id: cap_lang_ic_element_cap_inductor
* bench.group: capability
* bench.title: C1 ... ic=2 and L1 ... ic=1 with .tran ... uic and no .ic card set the starting state (tau = 1 us on both branches)
* bench.probes: v(a), v(b)
* bench.tags: linear, tran, uic, ic, silent_risk
* bench.note: ngspice-47 gives v(a) = 2 V * exp(-t/1us) (5 us: 13.5 mV) and v(b) = -R2 * i_L = -1 V * exp(-t/1us) (hand-checked); the .ic card is not used here, so this is the element-ic path alone
R1 a 0 1k
C1 a 0 1n ic=2
R2 b 0 1
L1 b 0 1u ic=1
.tran 10n 5u uic
.end
