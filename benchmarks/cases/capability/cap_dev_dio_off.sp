Diode with OFF flag (initial guess only; OP must be unchanged)
* bench.id: cap_dev_dio_off
* bench.group: capability
* bench.title: Diode with OFF flag (initial guess only; OP must be unchanged)
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.ref: reltol=1e-9 vntol=1e-12 abstol=1e-12
* bench.tags: diode, op, instance_param
* bench.note: ref abstol 1e-12: with OFF the 1e-15 reference makes ngspice fall through dynamic/true gmin stepping to source stepping (same OP); abstol 1e-12 converges directly, error < 1e-9 V
V1 in 0 5
R1 in a 1k
D1 a 0 DM off
.model DM D(IS=1e-14)
.op
.end
