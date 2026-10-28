RC discharge from a .ic node voltage
* bench.id: acc_tr_ic_dotic_uic
* bench.group: accuracy
* bench.title: RC (tau = 1 ms) discharging from .ic v(out)=5 with .tran ... UIC
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, ic, uic
* bench.est_rss_mib: 16
* bench.note: No oracle (uic unsupported); closed form v(out) = 5 exp(-t/1 ms) verified independently.
R1 out 0 1k
C1 out 0 1u
.ic v(out)=5
.tran 20u 3m UIC
.end
