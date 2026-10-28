subcircuit expansion bomb: 10^9 resistors from a 9-level fan-out-10 hierarchy
* bench.id: rob_hs_sub_expansion_bomb_1e9
* bench.group: robustness
* bench.title: subcircuit expansion bomb: 10^9 resistors from a 9-level fan-out-10 hierarchy
* bench.probes: v(in), v(a)
* bench.expect: either
* bench.tags: hostile, subckt, hierarchy, resource, ngspice_unbounded
* bench.est_rss_mib: 256
* bench.note: Legal but absurd input (40 lines). Independent count: 9 levels of fan-out 10 give exactly 10^9 leaf resistors of 1e12 ohm in parallel = 1 kohm, so v(a) = 0.5 V behind Rs = 1k and i(v1) = -0.5 mA; RustSpice solves the same hierarchy at 10^3..10^6 instances (leaf 1e(L+3) ohm) with v(a) within 5e-11 of 0.5, so the arithmetic is fine and only the missing instance budget is at issue. ngspice-47 is equally unbounded: 14/23/99 MiB at 10^3/10^4/10^5 instances and killed at the 2 GiB RSS cap after 7 s on the full deck (not run by the harness, tag ngspice_unbounded). A panic, OOM kill or timeout is a violation even for either (SUITE_SPEC section 5), so only a refusal flips this deck to gap_closed. est_rss_mib is the estimate of a correct (refusing) run, not of the observed runaway: the harness RSS cap bounds the real run.
.subckt l0 a b
R1 a b 1e12
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
.subckt l6 a b
X0 a b l5
X1 a b l5
X2 a b l5
X3 a b l5
X4 a b l5
X5 a b l5
X6 a b l5
X7 a b l5
X8 a b l5
X9 a b l5
.ends
.subckt l7 a b
X0 a b l6
X1 a b l6
X2 a b l6
X3 a b l6
X4 a b l6
X5 a b l6
X6 a b l6
X7 a b l6
X8 a b l6
X9 a b l6
.ends
.subckt l8 a b
X0 a b l7
X1 a b l7
X2 a b l7
X3 a b l7
X4 a b l7
X5 a b l7
X6 a b l7
X7 a b l7
X8 a b l7
X9 a b l7
.ends
.subckt l9 a b
X0 a b l8
X1 a b l8
X2 a b l8
X3 a b l8
X4 a b l8
X5 a b l8
X6 a b l8
X7 a b l8
X8 a b l8
X9 a b l8
.ends
V1 in 0 1
Rs in a 1k
Xp a 0 l9
.op
.end
