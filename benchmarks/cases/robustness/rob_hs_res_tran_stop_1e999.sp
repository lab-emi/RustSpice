tran 1n 1e999 (tstop overflows to infinity)
* bench.id: rob_hs_res_tran_stop_1e999
* bench.group: robustness
* bench.title: .tran 1n 1e999 (tstop overflows to infinity)
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, resource, tran, nonfinite, ngspice_unbounded
* bench.est_rss_mib: 64
* bench.note: Truth: an infinite tstop has no finite answer, so refusal is the only correct outcome. ngspice-47 warns "memory required (inf PB)" and runs on (not run by the harness, tag ngspice_unbounded). Both the non-finite input and the missing budget are at issue; the unbounded loop is the harm, hence resource-budget. A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1n 1e999
.end
