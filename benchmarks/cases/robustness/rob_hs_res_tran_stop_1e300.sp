tran 1 1e300 (1e300 time points)
* bench.id: rob_hs_res_tran_stop_1e300
* bench.group: robustness
* bench.title: .tran 1 1e300 (1e300 time points)
* bench.probes: v(2)
* bench.ref: none
* bench.expect: either
* bench.tags: hostile, resource, tran
* bench.est_rss_mib: 64
* bench.note: Truth: 1e300 time points, unfinishable, so refusal is the only correct outcome. ngspice-47 warns "memory required (2.8e286 PB)" and exits 1 within 10 ms, but through "Timestep too small; time = 1e-11" (a step-control failure just after the start), not through a budget refusal, so its exit code is not evidence of a clean refusal; ref is none because the injected reference tmax makes ngspice run away (20 s watchdog). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1n
.tran 1 1e300
.end
