capacitor value 1nF in a transient
* bench.id: rob_hs_val_unit_farad
* bench.group: robustness
* bench.title: capacitor value 1nF in a transient
* bench.probes: v(2)
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: hostile, value, units, parser, tran
* bench.est_rss_mib: 64
* bench.note: 1nF must be read as 1 nF (tau = 1 us)
V1 1 0 PULSE(0 5 0 1n 1n 1 2)
R1 1 2 1k
C1 2 0 1nF
.tran 100n 3u
.end
