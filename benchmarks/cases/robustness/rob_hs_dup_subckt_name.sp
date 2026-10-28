same .subckt name defined twice with different bodies
* bench.id: rob_hs_dup_subckt_name
* bench.group: robustness
* bench.title: same .subckt name defined twice with different bodies
* bench.probes: v(1), i(v1)
* bench.expect: either
* bench.tags: hostile, duplicate, parser, subckt
* bench.est_rss_mib: 64
* bench.note: undefined by the language: ngspice warns "redefinition of .subckt s, ignored" and keeps the FIRST body (i(v1) = -5 mA), RustSpice silently keeps the LAST (-1.667 mA). No SPICE convention makes either right: ambiguous
.subckt s a b
R1 a b 1k
.ends
.subckt s a b
R1 a b 3k
.ends
V1 1 0 5
X1 1 0 s
.op
.end
