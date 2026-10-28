Two capacitively coupled RC ladders (crosstalk)
* bench.id: acc_tr_coupled_rc_ladders
* bench.group: accuracy
* bench.title: Aggressor and victim RC ladders (8 sections each) coupled by 50 fF per section, 50 ps edge
* bench.probes: v(a8), v(v8), v(v0)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, crosstalk, coupled
* bench.est_rss_mib: 16
* bench.note: Mechanism check (same deck, tstep only changed): RustSpice's v(v0) error is 1.74e-2 at 10 ps, 4.8e-3 at 5 ps, 1.05e-3 at 2.5 ps and 1.7e-4 at 1 ps, i.e. second order, while ngspice as-is is 2.0e-3, 2.3e-3, 1.2e-3 and 3.6e-4 at the same steps; the errors measured against the tran_lti oracle (1.75e-2 and 1.96e-3 at 10 ps) equal those measured against the converged ngspice reference to better than 1e-4 of the swing.
Vagg ina 0 PULSE(0 1 0.1n 50p 50p 5n 10n)
Rsa ina a0 50
Rsv v0 0 50
Ra1 a0 a1 100
Ca1 a1 0 100f
Rv1 v0 v1 100
Cv1 v1 0 100f
Cc1 a1 v1 50f
Ra2 a1 a2 100
Ca2 a2 0 100f
Rv2 v1 v2 100
Cv2 v2 0 100f
Cc2 a2 v2 50f
Ra3 a2 a3 100
Ca3 a3 0 100f
Rv3 v2 v3 100
Cv3 v3 0 100f
Cc3 a3 v3 50f
Ra4 a3 a4 100
Ca4 a4 0 100f
Rv4 v3 v4 100
Cv4 v4 0 100f
Cc4 a4 v4 50f
Ra5 a4 a5 100
Ca5 a5 0 100f
Rv5 v4 v5 100
Cv5 v5 0 100f
Cc5 a5 v5 50f
Ra6 a5 a6 100
Ca6 a6 0 100f
Rv6 v5 v6 100
Cv6 v6 0 100f
Cc6 a6 v6 50f
Ra7 a6 a7 100
Ca7 a7 0 100f
Rv7 v6 v7 100
Cv7 v7 0 100f
Cc7 a7 v7 50f
Ra8 a7 a8 100
Ca8 a8 0 100f
Rv8 v7 v8 100
Cv8 v8 0 100f
Cc8 a8 v8 50f
Rla a8 0 10k
Rlv v8 0 10k
.tran 10p 3n
.end
