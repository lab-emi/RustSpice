Initial condition with spaces around the equals sign
* bench.id: acc_tr_ic_dotic_spaced
* bench.group: accuracy
* bench.title: RC (tau = 1 ms) discharging from ".ic V(out) = 5" (spaces around =) with .tran ... UIC
* bench.probes: v(out)
* bench.tol.norm: 5e-3
* bench.tags: linear, tran, ic, uic, silent_risk
* bench.est_rss_mib: 16
* bench.note: No oracle (uic unsupported); closed form v(out) = 5 exp(-t/1 ms) verified independently. The cause is the single-token .ic parser: '.ic V(out)=5' without spaces gives RustSpice an error of 2.0e-4 of swing; near-duplicate of cap_lang_ic_spaced_uic (same card, tau = 1 us there).
R1 out 0 1k
C1 out 0 1u
.ic V(out) = 5
.tran 20u 3m UIC
.end
