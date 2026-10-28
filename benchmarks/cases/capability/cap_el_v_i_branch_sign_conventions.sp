Branch-current sign conventions of V sources in series with every kind of element
* bench.id: cap_el_v_i_branch_sign_conventions
* bench.group: capability
* bench.title: Branch-current sign conventions of V sources in series with every kind of element
* bench.probes: i(v1), i(v2), i(v3), i(v4), i(v5), i(v6), i(v7), i(v8), v(c), v(d)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-15
* bench.floor: 1e-9
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: source, op, polarity, branch_current
* bench.note: SPICE counts i(v) positive when current flows into the + terminal, through the source and out of the - terminal. Hand values (ngspice-47 and the dc_linear oracle agree exactly): V1 a 0 5 carries its own 1k (5 mA) plus the 8 mA that V5 stacked on top of it sources: i(v1) = -13 mA. V2 has swapped terminals (0 b): v(b) = -5 V and i(v2) = -5 mA. V3 is a zero-volt ammeter that carries the 1 mA pushed into x by I1: i(v3) = +1 mA. V4 is an ammeter in the output of a VCCS (G1 0 o a 0 2m pushes 2m*5 V = 10 mA into o): i(v4) = +10 mA. V5 c a 3 drives 8 V into 1k: i(v5) = -8 mA. V6 is an ammeter in series with a current sink (I2 s 0 2m draws 2 mA out of s): i(v6) = -2 mA. V7 d 0 4 with V8 e d -1 stacked on it (e = 3 V): i(v8) = -1.5 mA (R4 2k), i(v7) = -2.5 mA (R5 4k plus the 1.5 mA of V8). Every probe is a non-zero exact rational of the netlist, so the tolerances are tight and a sign slip in any element type shows up.
V1 a 0 5
R1 a 0 1k
V2 0 b 5
R2 b 0 1k
V3 x 0 0
I1 0 x 1m
V4 o 0 0
G1 0 o a 0 2m
V5 c a 3
R3 c 0 1k
V6 s 0 0
I2 s 0 2m
V7 d 0 4
V8 e d -1
R4 e 0 2k
R5 d 0 4k
.op
.end
