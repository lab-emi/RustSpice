VCCS with gm=1e200 and a 1e200 V source: v(2) = 1e403 overflows
* bench.id: rob_hs_num_overflow_gain
* bench.group: robustness
* bench.title: VCCS with gm=1e200 and a 1e200 V source: v(2) = 1e403 overflows
* bench.probes: v(1)
* bench.expect: either
* bench.tags: hostile, numeric, overflow, nonfinite
* bench.est_rss_mib: 64
* bench.note: both tools print v(2)=inf with exit 0 (1e200*1e200*1k overflows; no finite answer exists) and agree on i(v1)=0 and v(1)=1e200; only the finite v(1) is compared, so the deck documents agreement on the non-finite result without judging it
V1 1 0 1e200
G1 0 2 1 0 1e200
R1 2 0 1k
.op
.end
