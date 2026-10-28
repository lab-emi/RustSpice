subcircuit nesting depth 10 (1 ohm per level), i(v1) = -1/10
* bench.id: rob_hs_sub_nest_10
* bench.group: robustness
* bench.title: subcircuit nesting depth 10 (1 ohm per level), i(v1) = -1/10
* bench.probes: v(1), i(v1)
* bench.expect: match
* bench.tags: subckt, hierarchy, dc
* bench.est_rss_mib: 64
* bench.note: closed form: total resistance 10 ohm, i(v1) = -0.1 A; internal node names are not compared (ngspice mangles them)
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
R1 a b 1
.ends
V1 1 0 1
Xtop 1 0 s0
.op
.end
