Five series diodes from 5 V through 1 kohm
* bench.id: cap_dev_dio_stack_op
* bench.group: capability
* bench.title: Five series diodes from 5 V through 1 kohm
* bench.probes: v(n1), v(n3), v(n5), i(v1)
* bench.expect: match
* bench.tags: diode, op, stack
V1 in 0 5
R1 in n1 1k
D1 n1 n2 DM
D2 n2 n3 DM
D3 n3 n4 DM
D4 n4 n5 DM
D5 n5 0 DM
.model DM D(IS=1e-14)
.op
.end
