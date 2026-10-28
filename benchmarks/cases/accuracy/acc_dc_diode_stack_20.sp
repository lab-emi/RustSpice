Series stack of 20 diodes (IS=1e-14) from 20 V through 1k
* bench.id: acc_dc_diode_stack_20
* bench.group: accuracy
* bench.title: Series stack of 20 diodes (IS=1e-14) from 20 V through 1k
* bench.probes: v(d1), v(d7), v(d14), v(d20), i(v1)
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.size: 22
* bench.tags: nonlinear, op, diode, stack
* bench.note: tol.abs tightened to 1e-12 A (the default 1e-9 A would be 1e-5 relative on mA currents)
V1 in 0 20
R1 in d1 1k
D1 d1 d2 dm
D2 d2 d3 dm
D3 d3 d4 dm
D4 d4 d5 dm
D5 d5 d6 dm
D6 d6 d7 dm
D7 d7 d8 dm
D8 d8 d9 dm
D9 d9 d10 dm
D10 d10 d11 dm
D11 d11 d12 dm
D12 d12 d13 dm
D13 d13 d14 dm
D14 d14 d15 dm
D15 d15 d16 dm
D16 d16 d17 dm
D17 d17 d18 dm
D18 d18 d19 dm
D19 d19 d20 dm
D20 d20 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
