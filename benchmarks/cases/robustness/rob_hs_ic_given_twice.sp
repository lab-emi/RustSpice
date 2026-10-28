ic v(2) given twice with uic (last value wins)
* bench.id: rob_hs_ic_given_twice
* bench.group: robustness
* bench.title: .ic v(2) given twice with uic (last value wins)
* bench.probes: v(2)
* bench.expect: match
* bench.tags: hostile, ic, tran
* bench.est_rss_mib: 64
* bench.note: closed form: v(2) = 4*exp(-t/1ms): 2.426 V at 500 us
V1 1 0 0
R1 1 2 1k
C1 2 0 1u
.ic v(2)=3
.ic v(2)=4
.tran 20u 500u uic
.end
