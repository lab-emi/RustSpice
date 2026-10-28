subcircuit nesting depth 22 (1 ohm per level), i(v1) = -1/22
* bench.id: rob_hs_sub_nest_22
* bench.group: robustness
* bench.title: subcircuit nesting depth 22 (1 ohm per level), i(v1) = -1/22
* bench.probes: v(1), i(v1)
* bench.expect: match
* bench.tags: subckt, hierarchy, dc, depth_limit
* bench.est_rss_mib: 64
* bench.note: depth 22 is the deepest nesting ngspice-47 runs (depths 1-19, 21 and 22 work; 20 fails with a false "infinite subckt recursion" and 23+ with "unknown subckt", see sub_nest_20 and sub_nest_50); closed form: 22 ohm in series, i(v1) = -1/22 = -0.0454545454545 A
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
R1 a b 1
.ends
V1 1 0 1
Xtop 1 0 s0
.op
.end
