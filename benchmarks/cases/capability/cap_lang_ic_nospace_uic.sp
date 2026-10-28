.ic without spaces and uic
* bench.id: cap_lang_ic_nospace_uic
* bench.group: capability
* bench.title: .ic v(out)=5 with .tran ... uic starts the RC discharge at 5 V (tau = 1 us)
* bench.probes: v(out)
* bench.tags: linear, tran, uic, ic
R1 out 0 1k
C1 out 0 1n
.ic v(out)=5
.tran 10n 5u uic
.end
