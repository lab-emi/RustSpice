tran 1n 1: 1e9 time points (24 GB of node data)
* bench.id: rob_hs_res_tran_1e9_steps
* bench.group: robustness
* bench.title: .tran 1n 1: 1e9 time points (24 GB of node data)
* bench.probes: v(2)
* bench.expect: either
* bench.tags: hostile, resource, tran, ngspice_unbounded
* bench.est_rss_mib: 64
* bench.note: Truth: closed-form RC step response, 1e9+1 points; RustSpice matches it at 1e5..1e7 steps within 5e-4 of the 5 V swing (fixed step, not at issue here). Finite work, so a refusal and a completed run are both correct (either). ngspice-47 warns "memory required (29.802 GB)" and runs on (14.4 MiB flat at 1e5 and 1e6 steps: it streams to the raw file; not run by the harness, tag ngspice_unbounded). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1n 1
.end
