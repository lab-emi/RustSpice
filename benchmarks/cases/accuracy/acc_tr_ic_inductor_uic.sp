RL decay from an inductor initial current
* bench.id: acc_tr_ic_inductor_uic
* bench.group: accuracy
* bench.title: Parallel RL (tau = 0.1 us) with L1 IC=1 A decaying with .tran ... UIC
* bench.probes: v(a)
* bench.tol.norm: 5e-3
* bench.known_gap: silent-ignore-ic
* bench.gap.note: Inductor IC=1 and UIC ignored: the decay starts from the DC operating point (v(a) = 0 V flat), error 1.0 of swing
* bench.tags: linear, tran, ic, uic, silent_risk
* bench.est_rss_mib: 16
* bench.note: No oracle (uic/ic= unsupported); closed form v(a) = -10 exp(-t/0.1 us) V verified independently.
L1 a 0 1u IC=1
R1 a 0 10
.tran 5n 500n UIC
.end
