RC driven by an EXP source
* bench.id: acc_tr_rc_exp_source
* bench.group: accuracy
* bench.title: RC (tau = 100 ns) driven by EXP(0 5 1u 0.5u 5u 1u): rising and decaying exponential stimulus
* bench.probes: v(out)
* bench.tol.norm: 1e-2
* bench.tags: linear, tran, exp, silent_risk
* bench.est_rss_mib: 16
* bench.note: Independent truth: scipy DOP853 (rtol 1e-13) solution of the RC ODE driven by the textbook EXP(0 5 1u 0.5u 5u 1u) formula. Tight ngspice (reltol 1e-9, tmax = tstep/20) is within 1.3e-6 of swing of it at its own rows (1.9e-6 after resampling to the tstep grid, 6.2e-6 for the harness reference); as-is ngspice is 3.7e-3 at its own rows (4.6e-3 after the harness resampling), below tol.norm 1e-2; RustSpice stays at 0 V (error 1.0). A short EXP(0 1 1u) is also DC 0 V in RustSpice [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 EXP(0 5 1u 0.5u 5u 1u)
R1 in out 1k
C1 out 0 100p
.tran 50n 12u
.end
