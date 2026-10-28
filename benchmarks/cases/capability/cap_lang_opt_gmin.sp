Large gmin option on a diode circuit
* bench.id: cap_lang_opt_gmin
* bench.group: capability
* bench.title: .options gmin=1e-3 puts 1 mS across the junction and visibly lowers the diode voltage
* bench.probes: v(out)
* bench.oracle: dc_nonlinear(gmin=1e-3)
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .options gmin ignored: 0.6929 V instead of 0.6884 V
* bench.tags: nonlinear, op, options, gmin
* bench.note: independent truth (mpmath, 40 digits, Shockley + gmin across the junction): 0.688415309 V with gmin = 1e-3 and 0.692887832 V with the default 1e-12; tightly converged ngspice 0.688415078 V; RustSpice 0.692887599 V is the default-gmin value and its output is bit-identical with the .options line deleted. The engine oracle dc_nonlinear(gmin=1e-3) reports 'unsupported' for decks that carry .options gmin, so the truth above is the independent check
.options gmin=1e-3
V1 in 0 DC 5
R1 in out 1k
D1 out 0 DM
.model DM D(IS=1e-14)
.op
.end
