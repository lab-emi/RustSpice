Inductor current decaying through a series diode after a voltage pulse ends
* bench.id: cap_xm_l_tran_diode_decay
* bench.group: capability
* bench.title: 3 V pulse into R-L-diode: the inductor current commutates into the diode loop when the pulse ends, tstep 0.5 ns
* bench.probes: v(a), i(l1), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.tags: passive, inductor, tran, diode, pulse, npoints
* bench.est_rss_mib: 16
* bench.note: L x .tran x diode without a failure (the only other corpus transients with L and a diode are the buck/boost converter decks, which are known gaps): current ramps to 58 mA through R1/L1/D1 (v(a) down to -1.74 V after the pulse falls) and decays through the diode loop closed by the 0 V source. v(b) is NOT a probe: once the current has died out (t > ~190 ns, nA-level diode current) v(b) is a stiff ill-conditioned state on which the tools disagree by 40 % of its 0.76 V swing (converged ngspice +-0.03 V, ngspice as-is +-0.33 V, RustSpice +0.31 V, continuous solution ~0), and v(a)/i(l1) are insensitive to it (<= 1.5e-9 A). Independent oracle (state = diode voltage, L*Id(vb)'*vb' = vin - R1*Id(vb) - vb, scipy Radau rtol 1e-11, restarted at the pulse corners): converged ngspice 6e-8 / 1.6e-7 of swing from it; RustSpice 2.3e-4 (v(a)) and 6.2e-4 (i(l1)) at 0.5 ns, 4.5e-5 / 1.2e-4 at 0.25 ns, 6.5e-4 / 1.8e-3 at 1 ns, 5.3e-3 / 1.4e-2 at 2 ns (second order); ngspice as-is only 1.0e-5 / 2.8e-5 (LTE control and breakpoints), so at this step RustSpice is ~20x less accurate than ngspice here: a fixed-step-accuracy effect that stays 3x below tol.norm, hence not tagged as a gap.
V1 in 0 PULSE(0 3 10n 1n 1n 100n 300n)
R1 in a 30
L1 a b 2u
D1 b 0 dm
.model dm D(IS=1e-14 N=1)
.tran 0.5n 300n
.end
