negative resistor R2 = -3k, v(2) = 7.5 V
* bench.id: rob_hs_r_negative_resistor
* bench.group: robustness
* bench.title: negative resistor R2 = -3k, v(2) = 7.5 V
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: hostile, value, r_negative, dc, silent_risk
* bench.est_rss_mib: 64
* bench.note: closed form v(2) = 5*(-3k)/(1k-3k) = 7.5 V and i(v1) = +2.5 mA, both reproduced by ngspice-47. Same circuit as rob_cv_res_negative_op (duplicate witness: counted twice in the gap tally).
V1 1 0 5
R1 1 2 1k
R2 2 0 -3k
.op
.end
