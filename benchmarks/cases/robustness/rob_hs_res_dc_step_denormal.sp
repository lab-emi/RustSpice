dc V1 0 1 1e-320 (denormal step)
* bench.id: rob_hs_res_dc_step_denormal
* bench.group: robustness
* bench.title: .dc V1 0 1 1e-320 (denormal step)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, resource, dc, ngspice_unbounded
* bench.est_rss_mib: 64
* bench.note: Truth: (1-0)/1e-320 = 1e320 sweep points, more than any count can represent, so refusal is the only correct outcome; the deck is left at either (informational) because ngspice is not run (tag ngspice_unbounded; it streams and is still running after 8 s at 14.7 MiB). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 0 1 1e-320
.end
