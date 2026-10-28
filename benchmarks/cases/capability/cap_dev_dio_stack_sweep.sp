Four series diodes, 0..4 V sweep
* bench.id: cap_dev_dio_stack_sweep
* bench.group: capability
* bench.title: Four series diodes, 0..4 V sweep
* bench.probes: v(n1), v(n3), i(v1)
* bench.expect: match
* bench.tags: diode, dc, stack
V1 in 0 0
R1 in n1 1k
D1 n1 n2 DM
D2 n2 n3 DM
D3 n3 n4 DM
D4 n4 0 DM
.model DM D(IS=1e-14)
.dc V1 0 4 0.1
.end
