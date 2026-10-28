Series stack of 70 diodes (IS=1e-14) from 50 V through 1k: 72 unknowns, 47.5 V across the stack
* bench.id: acc_dc_diode_stack_70
* bench.group: accuracy
* bench.title: Series stack of 70 diodes (IS=1e-14) from 50 V through 1k: 72 unknowns, 47.5 V across the stack
* bench.probes: v(d1), v(d24), v(d47), v(d70), i(v1)
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.size: 72
* bench.tags: nonlinear, op, diode, stack
* bench.note: tol.abs tightened to 1e-12 A (the default 1e-9 A would be 1e-5 relative on mA currents)
V1 in 0 50
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
D20 d20 d21 dm
D21 d21 d22 dm
D22 d22 d23 dm
D23 d23 d24 dm
D24 d24 d25 dm
D25 d25 d26 dm
D26 d26 d27 dm
D27 d27 d28 dm
D28 d28 d29 dm
D29 d29 d30 dm
D30 d30 d31 dm
D31 d31 d32 dm
D32 d32 d33 dm
D33 d33 d34 dm
D34 d34 d35 dm
D35 d35 d36 dm
D36 d36 d37 dm
D37 d37 d38 dm
D38 d38 d39 dm
D39 d39 d40 dm
D40 d40 d41 dm
D41 d41 d42 dm
D42 d42 d43 dm
D43 d43 d44 dm
D44 d44 d45 dm
D45 d45 d46 dm
D46 d46 d47 dm
D47 d47 d48 dm
D48 d48 d49 dm
D49 d49 d50 dm
D50 d50 d51 dm
D51 d51 d52 dm
D52 d52 d53 dm
D53 d53 d54 dm
D54 d54 d55 dm
D55 d55 d56 dm
D56 d56 d57 dm
D57 d57 d58 dm
D58 d58 d59 dm
D59 d59 d60 dm
D60 d60 d61 dm
D61 d61 d62 dm
D62 d62 d63 dm
D63 d63 d64 dm
D64 d64 d65 dm
D65 d65 d66 dm
D66 d66 d67 dm
D67 d67 d68 dm
D68 d68 d69 dm
D69 d69 d70 dm
D70 d70 0 dm
.model dm D(IS=1e-14 N=1)
.op
.end
