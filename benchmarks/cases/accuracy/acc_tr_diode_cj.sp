Reverse-biased diode junction capacitance in a transient
* bench.id: acc_tr_diode_cj
* bench.group: accuracy
* bench.title: 5 V step through 1 k into a reverse-biased diode: the out node charges through the 2 pF zero-bias junction capacitance (CJO)
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: RustSpice keeps CJO as a constant 2 pF and ignores the VJ/M grading (bit-identical without VJ/M): v(out) crosses 2.5 V at 2.64 ns instead of 2.10 ns (0.28 of swing off); no warning
* bench.tags: nonlinear, tran, diode, junction_cap, silent_risk
* bench.est_rss_mib: 16
* bench.note: independent ODE oracle (scipy Radau, rtol 1e-10) with the graded C(v) = CJO/(1+v/VJ)^M crosses 2.5 V at 2.1042 ns, equal to converged ngspice (2.1042 ns, norm err 1.3e-5); the same ODE with C held at 2 pF crosses at 2.6415 ns, which is what RustSpice gives (2.6422 ns); without CJO the node follows the source (1.25 ns); so the RustSpice error is the missing grading, not a missing capacitor
V1 in 0 PULSE(0 5 1n 0.5n 0.5n 20n 40n)
R1 in out 1k
D1 0 out DMOD
.model DMOD D(IS=1e-14 N=1.0 CJO=2p VJ=0.7 M=0.5)
.tran 0.1n 30n
.end
