RC driven by a piecewise-linear source
* bench.id: acc_tr_rc_pwl
* bench.group: accuracy
* bench.title: RC (tau = 1 us) driven by a PWL ramp-hold-fall-step stimulus
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, pwl, silent_risk
* bench.est_rss_mib: 16
* bench.note: Independent truth: the tran_lti oracle (and a scipy solution of the RC response split at the PWL breakpoints): tight ngspice (reltol 1e-9, tmax = tstep/20) is within 8.5e-8 of swing of it at its own rows (2.2e-7 after resampling to the tstep grid, 1.3e-6 for the harness reference), as-is ngspice 4.8e-5 at its own rows (1.5e-4 after the harness resampling); RustSpice 1.0. A triangle stimulus rewritten as PULSE(0 1 0 1u 1u 0 2u) is read correctly by RustSpice (3.7e-14 of swing against ngspice) while the same triangle as PWL is DC 0 V (error 1.0), so only the PWL card is dropped. Caveat: the 10 ns step 8u -> 8.01u lies inside the 50 ns grid; a PULSE with the same knee (0.5 -> 1.5 V in 10 ns) driving the same RC differs between RustSpice and ngspice by about 1.0e-2 of this deck's swing (above tol.norm 5e-3), so parsing PWL may leave this deck open through the fixed-step edge error [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 in 0 PWL(0 0 1u 0 3u 2 6u 2 8u 0.5 8.01u 1.5 12u 1.5)
R1 in out 1k
C1 out 0 1n
.tran 50n 12u
.end
