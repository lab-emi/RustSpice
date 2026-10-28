.ic on a subcircuit-internal node
* bench.id: cap_lang_ic_internal_node
* bench.group: capability
* bench.title: .ic v(x1.cap)=3 initialises the capacitor node inside X1
* bench.probes: v(x1.cap)
* bench.tags: linear, tran, uic, ic, subckt
.subckt RCCELL ref
R1 cap ref 1k
C1 cap ref 1n
.ends RCCELL
X1 0 RCCELL
.ic v(x1.cap)=3
.tran 10n 5u uic
.end
