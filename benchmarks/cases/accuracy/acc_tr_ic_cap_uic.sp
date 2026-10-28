RC discharge from an element initial condition
* bench.id: acc_tr_ic_cap_uic
* bench.group: accuracy
* bench.title: RC (tau = 1 ms) discharging from C1 IC=5 with .tran ... UIC
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.known_gap: silent-ignore-ic
* bench.gap.note: Element IC=5 and UIC ignored: the capacitor starts from the DC operating point (v(out) = 0 V flat), error 1.05 of swing at t = 0
* bench.tags: linear, tran, ic, uic, silent_risk
* bench.est_rss_mib: 16
* bench.note: No oracle (uic/ic= unsupported); closed form v(out) = 5 exp(-t/1 ms) verified independently (ngspice 1e-4).
R1 out 0 1k
C1 out 0 1u IC=5
.tran 20u 3m UIC
.end
