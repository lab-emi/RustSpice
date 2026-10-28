tstep larger than tstop
* bench.id: acc_tr_tstep_gt_tstop
* bench.group: accuracy
* bench.title: RC (tau = 1 ps) driven by a 1 V step with .tran 1u 1n (tstep > tstop): rows at 0 and 1 ns, out = 0 then 1 V
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, final_time
* bench.est_rss_mib: 16
V1 in 0 PULSE(0 1 0 1p 1p 1 2)
R1 in out 1
C1 out 0 1p
.tran 1u 1n
.end
