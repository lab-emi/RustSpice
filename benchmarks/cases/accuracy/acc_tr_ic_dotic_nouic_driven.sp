Initial condition on a driven RC without UIC
* bench.id: acc_tr_ic_dotic_nouic_driven
* bench.group: accuracy
* bench.title: RC (tau = 1 ms) from a 1 V source with .ic v(out)=5 and no UIC: the operating point is solved with out held at 5 V, then released
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, ic, silent_risk
* bench.est_rss_mib: 16
V1 in 0 DC 1
R1 in out 1k
C1 out 0 1u
.ic v(out)=5
.tran 20u 3m
.end
