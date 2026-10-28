Transient with uic and no initial conditions
* bench.id: cap_lang_tran_uic_no_ic
* bench.group: capability
* bench.title: .tran ... uic skips the DC operating point: a 5 V source charging an RC (tau = 1 us) starts from 0 V
* bench.probes: v(out)
* bench.known_gap: silent-ignore-ic
* bench.gap.note: uic ignored: the DC operating point (5 V) is used instead of starting from 0 V
* bench.tags: linear, tran, uic
V1 in 0 DC 5
R1 in out 1k
C1 out 0 1n
.tran 10n 5u uic
.end
