dc V1 0 1 1e-9: 1e9 sweep points
* bench.id: rob_hs_res_dc_1e9_points
* bench.group: robustness
* bench.title: .dc V1 0 1 1e-9: 1e9 sweep points
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, resource, dc, ngspice_unbounded
* bench.est_rss_mib: 64
* bench.note: Truth: 1e9+1 points with v(2) = V/2 and i(v1) = -V/2000, finite work, so a refusal and a completed run are both correct (either). ngspice-47 streams the points to the raw file: 14.5 MiB flat at 1e5 and 1e6 points, and still running after 8 s on this deck (not run by the harness, tag ngspice_unbounded). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.dc V1 0 1 1e-9
.end
