subckt defined inside another .subckt
* bench.id: rob_hs_sub_nested_definition
* bench.group: robustness
* bench.title: .subckt defined inside another .subckt
* bench.probes: v(1), i(v1)
* bench.expect: match
* bench.tags: subckt, parser, capability
* bench.est_rss_mib: 64
* bench.note: legal in ngspice: the inner definition is local to outer, i(v1) = -5 mA = -5 V/1k (verified)
.subckt outer a b
.subckt inner x y
R1 x y 1k
.ends inner
X1 a b inner
.ends outer
V1 1 0 5
Xo 1 0 outer
.op
.end
