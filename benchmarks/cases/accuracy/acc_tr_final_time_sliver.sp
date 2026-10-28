tstop whose last step is a float sliver
* bench.id: acc_tr_final_time_sliver
* bench.group: accuracy
* bench.title: Sine into RC with .tran 10n 100u (tstop/tstep not an exact binary multiple): no spurious ~1e-17 s final step
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: linear, tran, final_time, npoints
* bench.est_rss_mib: 32
* bench.note: Audit a3 F16. 100u/10n is exactly 10000 steps, so floor(tstop/tstep)+1 = 10001 rows is the criterion; ngspice as-is writes 10008 rows only because of its start-up sub-steps (1e-10..6.4e-9 s) and a tail adjustment, its time grid has no sliver (smallest late dt 8.6e-9 s, last time = tstop - 1e-20). The value of the extra row is benign here (C/h = 8e7 S is easily handled); the stiff variant with garbage values is acc_tr_last_step_rc_mesh. The sliver depends on accumulated rounding of t += h against the 1e-9*tstep guard, not on this deck: the same RC with .tran 10n 100u, 5n 100u, 4n 100u and 3n 300u (1e4 to 1e5 steps) writes one extra row with dt 1.3e-17 to 2.8e-16 s, while .tran 20n 100u, 2n 100u and 1n 10u write exactly floor(tstop/tstep)+1 rows (measured).
V1 in 0 SIN(0 1 1Meg)
R1 in out 1k
C1 out 0 1n
.tran 10n 100u
.end
