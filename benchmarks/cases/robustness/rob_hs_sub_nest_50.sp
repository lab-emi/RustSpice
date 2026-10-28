subcircuit nesting depth 50 (1 ohm per level), i(v1) = -1/50
* bench.id: rob_hs_sub_nest_50
* bench.group: robustness
* bench.title: subcircuit nesting depth 50 (1 ohm per level), i(v1) = -1/50
* bench.probes: v(1), i(v1)
* bench.expect: either
* bench.tags: subckt, hierarchy, dc, ngspice_crash
* bench.est_rss_mib: 64
* bench.note: ngspice-47 cannot run this deck (deterministic: nesting of 23 or more levels gives "unknown subckt: xtop.x1...x1.m s22" and, under -r, a segfault; see sub_nest_22 and sub_nest_20 for its limits). Closed form: 50 x 1 ohm in series, i(v1) = -1/50 = -0.02 A; RustSpice returns -0.02000000000000031 (1.5e-14 relative), so RustSpice is right and ngspice crashes. The engine cannot judge against a closed form when ngspice fails, hence expect either (info / ngspice_only_error)
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
X1 a m s20
R1 m b 1
.ends
.subckt s20 a b
X1 a m s21
R1 m b 1
.ends
.subckt s21 a b
X1 a m s22
R1 m b 1
.ends
.subckt s22 a b
X1 a m s23
R1 m b 1
.ends
.subckt s23 a b
X1 a m s24
R1 m b 1
.ends
.subckt s24 a b
X1 a m s25
R1 m b 1
.ends
.subckt s25 a b
X1 a m s26
R1 m b 1
.ends
.subckt s26 a b
X1 a m s27
R1 m b 1
.ends
.subckt s27 a b
X1 a m s28
R1 m b 1
.ends
.subckt s28 a b
X1 a m s29
R1 m b 1
.ends
.subckt s29 a b
X1 a m s30
R1 m b 1
.ends
.subckt s30 a b
X1 a m s31
R1 m b 1
.ends
.subckt s31 a b
X1 a m s32
R1 m b 1
.ends
.subckt s32 a b
X1 a m s33
R1 m b 1
.ends
.subckt s33 a b
X1 a m s34
R1 m b 1
.ends
.subckt s34 a b
X1 a m s35
R1 m b 1
.ends
.subckt s35 a b
X1 a m s36
R1 m b 1
.ends
.subckt s36 a b
X1 a m s37
R1 m b 1
.ends
.subckt s37 a b
X1 a m s38
R1 m b 1
.ends
.subckt s38 a b
X1 a m s39
R1 m b 1
.ends
.subckt s39 a b
X1 a m s40
R1 m b 1
.ends
.subckt s40 a b
X1 a m s41
R1 m b 1
.ends
.subckt s41 a b
X1 a m s42
R1 m b 1
.ends
.subckt s42 a b
X1 a m s43
R1 m b 1
.ends
.subckt s43 a b
X1 a m s44
R1 m b 1
.ends
.subckt s44 a b
X1 a m s45
R1 m b 1
.ends
.subckt s45 a b
X1 a m s46
R1 m b 1
.ends
.subckt s46 a b
X1 a m s47
R1 m b 1
.ends
.subckt s47 a b
X1 a m s48
R1 m b 1
.ends
.subckt s48 a b
X1 a m s49
R1 m b 1
.ends
.subckt s49 a b
R1 a b 1
.ends
V1 1 0 1
Xtop 1 0 s0
.op
.end
