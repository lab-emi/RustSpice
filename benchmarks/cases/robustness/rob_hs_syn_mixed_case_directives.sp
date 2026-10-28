mixed-case directives and subcircuit names
* bench.id: rob_hs_syn_mixed_case_directives
* bench.group: robustness
* bench.title: mixed-case dot directives, subcircuit and model names
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: syntax, parser, subckt, model, lenient_ok
* bench.est_rss_mib: 64
* bench.note: .SuBcKt Foo is instantiated as fOO and closed by .EnDs FOO; .MoDeL dm with is=1e-12 is referenced as DM. Non-vacuous: R1 inside Foo is the series resistor of the diode, so v(2)=0.5745 V; a dropped model parameter (default IS) would give 0.6929 V and a skipped X1 would leave the diode unpowered (v(2)=0)
.SuBcKt Foo a b
R1 a b 1k
.EnDs FOO
V1 1 0 5
X1 1 2 fOO
.MoDeL dm D(Is=1e-12)
D9 2 0 DM
.Op
.EnD
