tran 10n 2u 0 0 (tmax = 0 means default)
* bench.id: rob_hs_tran_tmax_zero
* bench.group: robustness
* bench.title: .tran 10n 2u 0 0 (tmax = 0 means default)
* bench.probes: v(2)
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: tran, args, lenient_ok
* bench.est_rss_mib: 64
V1 1 0 SIN(0 1 1meg)
R1 1 2 1k
C1 2 0 100p
.tran 10n 2u 0 0
.end
