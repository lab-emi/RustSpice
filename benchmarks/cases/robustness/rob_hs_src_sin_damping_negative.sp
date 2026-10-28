SIN with negative damping (exponential growth e^(+1e7 t))
* bench.id: rob_hs_src_sin_damping_negative
* bench.group: robustness
* bench.title: SIN with negative damping (exponential growth e^(+1e7 t))
* bench.probes: v(2)
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: hostile, source, tran
* bench.est_rss_mib: 64
V1 1 0 SIN(0 1 1meg 0 -1e7)
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u
.end
