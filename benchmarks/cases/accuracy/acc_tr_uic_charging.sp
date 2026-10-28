UIC without any initial-condition value
* bench.id: acc_tr_uic_charging
* bench.group: accuracy
* bench.title: RC (tau = 1 ms) driven by a 1 V DC source with .tran ... UIC and no IC: the capacitor must start at 0 V and charge
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, uic, silent_risk
* bench.est_rss_mib: 16
* bench.note: No oracle (uic unsupported); closed form v(out) = 1 - exp(-t/1 ms) verified independently.
V1 in 0 DC 1
R1 in out 1k
C1 out 0 1u
.tran 20u 3m UIC
.end
