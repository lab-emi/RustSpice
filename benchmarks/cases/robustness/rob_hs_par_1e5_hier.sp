case: 100000 parallel 1e8 ohm resistors via a 5-level x10 hierarchy, 40 lines
* bench.id: rob_hs_par_1e5_hier
* bench.group: robustness
* bench.title: 100000 parallel 1e8 ohm resistors via a 5-level x10 hierarchy, 40 lines
* bench.probes: v(in), v(a), i(v1)
* bench.expect: match
* bench.tags: subckt, hierarchy, scale, dc
* bench.est_rss_mib: 100
* bench.note: closed form: 1e5 x 1e8 ohm in parallel = 1 kohm, v(a) = 0.5 V, i(v1) = -0.5 mA
.subckt l0 a b
R1 a b 1e8
.ends
.subckt l1 a b
X0 a b l0
X1 a b l0
X2 a b l0
X3 a b l0
X4 a b l0
X5 a b l0
X6 a b l0
X7 a b l0
X8 a b l0
X9 a b l0
.ends
.subckt l2 a b
X0 a b l1
X1 a b l1
X2 a b l1
X3 a b l1
X4 a b l1
X5 a b l1
X6 a b l1
X7 a b l1
X8 a b l1
X9 a b l1
.ends
.subckt l3 a b
X0 a b l2
X1 a b l2
X2 a b l2
X3 a b l2
X4 a b l2
X5 a b l2
X6 a b l2
X7 a b l2
X8 a b l2
X9 a b l2
.ends
.subckt l4 a b
X0 a b l3
X1 a b l3
X2 a b l3
X3 a b l3
X4 a b l3
X5 a b l3
X6 a b l3
X7 a b l3
X8 a b l3
X9 a b l3
.ends
.subckt l5 a b
X0 a b l4
X1 a b l4
X2 a b l4
X3 a b l4
X4 a b l4
X5 a b l4
X6 a b l4
X7 a b l4
X8 a b l4
X9 a b l4
.ends
V1 in 0 1
Rs in a 1k
Xp a 0 l5
.op
.end
