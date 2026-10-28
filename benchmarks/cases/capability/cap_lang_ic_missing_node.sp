.ic on a node that does not exist
* bench.id: cap_lang_ic_missing_node
* bench.group: capability
* bench.title: .ic v(out)=4 v(nosuchnode)=3: the entry for the absent node must not disturb the valid one (V1 = 1 V, tau = 1 ms)
* bench.probes: v(out)
* bench.tags: linear, tran, ic, invalid_input
* bench.note: ngspice-47 prints 'IC on non-existent node ... ignored' and continues
V1 in 0 DC 1
R1 in out 1k
C1 out 0 1u
.ic v(out)=4 v(nosuchnode)=3
.tran 20u 1m
.end
