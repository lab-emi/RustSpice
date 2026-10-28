tran 1p 1: 1e12 time points
* bench.id: rob_hs_res_tran_1e12_steps_1p_1s
* bench.group: robustness
* bench.title: .tran 1p 1: 1e12 time points
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, resource, tran, ngspice_unbounded
* bench.est_rss_mib: 64
* bench.note: Truth: 1e12 time points (ngspice estimates 29.1 TB of node data), finite but unfinishable work, so refusal is the only practical outcome. ngspice-47 warns "memory required (29.104 TB)" and then runs until killed (not run by the harness, tag ngspice_unbounded). Same ratio and code path as rob_hs_res_tran_1e12_steps_1n_1000s (a redundant pair: only the time scale differs). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1p 1
.end
