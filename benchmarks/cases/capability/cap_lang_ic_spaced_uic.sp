.ic with spaces around the equals sign
* bench.id: cap_lang_ic_spaced_uic
* bench.group: capability
* bench.title: .ic V(out) = 5 (spaces) with .tran ... uic
* bench.probes: v(out)
* bench.known_gap: silent-ignore-ic
* bench.gap.note: .ic V(out) = 5 (spaces) is ignored: starts from 0 V
* bench.tags: linear, tran, uic, ic, syntax
R1 out 0 1k
C1 out 0 1n
.ic V(out) = 5
.tran 10n 5u uic
.end
