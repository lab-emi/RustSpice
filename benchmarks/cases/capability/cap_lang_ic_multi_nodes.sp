.ic with several nodes
* bench.id: cap_lang_ic_multi_nodes
* bench.group: capability
* bench.title: .ic v(a)=4 v(b)=2 initialises two capacitors on one card
* bench.probes: v(a), v(b)
* bench.tags: linear, tran, uic, ic
R1 a 0 1k
C1 a 0 1n
R2 b 0 2k
C2 b 0 1n
.ic v(a)=4 v(b)=2
.tran 10n 5u uic
.end
