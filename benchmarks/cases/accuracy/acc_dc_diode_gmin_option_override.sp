Same two reverse diodes with .options gmin=1e-9: the middle node moves to 2.5002 V
* bench.id: acc_dc_diode_gmin_option_override
* bench.group: accuracy
* bench.title: Same two reverse diodes with .options gmin=1e-9: the middle node moves to 2.5002 V
* bench.probes: v(m), i(v1)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.expect: match
* bench.known_gap: silent-ignore-directive
* bench.gap.note: .options gmin is accepted and ignored: RS keeps gmin = 1e-12 and returns v(m) = 2.7250 V instead of 2.5002 V (silent)
* bench.oracle: dc_nonlinear(gmin=1e-9)
* bench.tags: nonlinear, op, diode, gmin, option, silent_risk
* bench.note: v(m) = 2.5 + (IS1 - IS2) / (2 gmin) = 2.500225 V; tol.abs lowered to 1e-18 A. Independent truth (mpmath, 40 digits, Shockley + gmin across each junction): 2.500225 V with gmin = 1e-9 and 2.725 V with the default 1e-12; tightly converged ngspice 2.5002250 V. RustSpice output is bit-identical with the .options line deleted and equals ngspice with the option deleted (2.7249995 V, i(v1) magnitude 2.8e-12 A against 2.5e-9 A with the option). The engine oracle dc_nonlinear(gmin=1e-9) reports 'unsupported' for decks that carry .options gmin, so the truth above is the independent check
V1 in 0 5
D1 m in da
D2 0 m db
.model da D(IS=5e-13 N=1)
.model db D(IS=5e-14 N=1)
.options gmin=1e-9
.op
.end
