tstop not a multiple of tstep
* bench.id: acc_tr_final_time_partial
* bench.group: accuracy
* bench.title: Sine into a fast RC with .tran 1u 10.5u: the last interval is half a step and the sine is rising at the end
* bench.probes: v(in), v(out)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, final_time, npoints
* bench.est_rss_mib: 16
* bench.note: A correct simulator ends exactly at tstop (10.5 us); one that stops at 10 us misses a 0.31 V rise (15 % of the swing). Expected row count 12 (11 steps + t=0), never more.
V1 in 0 SIN(0 1 100k)
R1 in out 100
C1 out 0 10p
.tran 1u 10.5u
.end
