100 series diodes from 1 kV through 100 ohm (9.1 A, 45 V across the middle)
* bench.id: rob_cv_diode_string_100_1kv_op
* bench.group: robustness
* bench.title: 100 series diodes from 1 kV through 100 ohm (9.1 A, 45 V across the middle)
* bench.probes: v(d1), v(d50), v(d100), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.size: 100
* bench.tags: nonlinear, diode, large_current, op
* bench.note: ngspice-47 converges directly (no stepping); my numpy Newton agrees to 1e-9 V (v(d50)=45.43738 V). RustSpice cannot reach its absolute 1e-12 A residual at 9 A. Calibration evidence: independent dense f64 Newton (gmin->0) matches ngspice-tight to 1e-12 V, with its own KCL residual 5e-12 A (the same rounding noise: largest term gd*v = 3.1e4 A, ulp 3.6e-12). Minimal variants: strings whose largest gd*v term is <= 7.7e3 A converge (10 diodes/200 V/10 ohm, 20/200 V/50 ohm, 50/500 V/100 ohm, 100/150 V/100 ohm) while 100 diodes at 300 V (7e3 A) and at 1 kV (3e4 A) fail, so the cliff is where the term's ulp passes 1e-12 A, not the topology.
V1 n0 0 1000
R1 n0 d1 100
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
D70 d70 d71 dm
D71 d71 d72 dm
D72 d72 d73 dm
D73 d73 d74 dm
D74 d74 d75 dm
D75 d75 d76 dm
D76 d76 d77 dm
D77 d77 d78 dm
D78 d78 d79 dm
D79 d79 d80 dm
D80 d80 d81 dm
D81 d81 d82 dm
D82 d82 d83 dm
D83 d83 d84 dm
D84 d84 d85 dm
D85 d85 d86 dm
D86 d86 d87 dm
D87 d87 d88 dm
D88 d88 d89 dm
D89 d89 d90 dm
D90 d90 d91 dm
D91 d91 d92 dm
D92 d92 d93 dm
D93 d93 d94 dm
D94 d94 d95 dm
D95 d95 d96 dm
D96 d96 d97 dm
D97 d97 d98 dm
D98 d98 d99 dm
D99 d99 d100 dm
D100 d100 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
