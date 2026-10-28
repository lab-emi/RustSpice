subcircuit nesting depth 20 (1 ohm per level), i(v1) = -1/20
* bench.id: rob_hs_sub_nest_20
* bench.group: robustness
* bench.title: subcircuit nesting depth 20 (1 ohm per level), i(v1) = -1/20
* bench.probes: v(1), i(v1)
* bench.expect: either
* bench.tags: subckt, hierarchy, dc, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 rejects exactly this depth with "Error: infinite subckt recursion" (a false positive: nothing recurses; depths 19, 21 and 22 of the identical structure run, and the message is independent of subckt/instance names) and segfaults under -r; RustSpice returns i(v1) = -0.05000000000000009 A, the closed form for 20 x 1 ohm is -0.05 A. RustSpice right, ngspice-only failure: expect either (info / ngspice_only_error) because the engine has no closed-form judge when ngspice fails
.subckt s0 a b
X1 a m s1
R1 m b 1
.ends
.subckt s1 a b
X1 a m s2
R1 m b 1
.ends
.subckt s2 a b
X1 a m s3
R1 m b 1
.ends
.subckt s3 a b
X1 a m s4
R1 m b 1
.ends
.subckt s4 a b
X1 a m s5
R1 m b 1
.ends
.subckt s5 a b
X1 a m s6
R1 m b 1
.ends
.subckt s6 a b
X1 a m s7
R1 m b 1
.ends
.subckt s7 a b
X1 a m s8
R1 m b 1
.ends
.subckt s8 a b
X1 a m s9
R1 m b 1
.ends
.subckt s9 a b
X1 a m s10
R1 m b 1
.ends
.subckt s10 a b
X1 a m s11
R1 m b 1
.ends
.subckt s11 a b
X1 a m s12
R1 m b 1
.ends
.subckt s12 a b
X1 a m s13
R1 m b 1
.ends
.subckt s13 a b
X1 a m s14
R1 m b 1
.ends
.subckt s14 a b
X1 a m s15
R1 m b 1
.ends
.subckt s15 a b
X1 a m s16
R1 m b 1
.ends
.subckt s16 a b
X1 a m s17
R1 m b 1
.ends
.subckt s17 a b
X1 a m s18
R1 m b 1
.ends
.subckt s18 a b
X1 a m s19
R1 m b 1
.ends
.subckt s19 a b
R1 a b 1
.ends
V1 1 0 1
Xtop 1 0 s0
.op
.end
