subckt card without a name
* bench.id: rob_hs_syn_subckt_no_name
* bench.group: robustness
* bench.title: .subckt card without a name
* bench.probes: v(1), v(2), i(v1)
* bench.expect: either
* bench.tags: hostile, syntax, parser, subckt
* bench.est_rss_mib: 64
* bench.note: AMBIGUOUS, no verdict (SUITE_SPEC 14.2 rule 2e). ngspice-47 accepts the nameless .subckt without any message and solves the 5 V divider (rc 0, v(2)=2.5 V, the body R1 never enters the circuit); RustSpice refuses ('.subckt requires a name and at least one port'). A nameless definition can never be instantiated, so refusing is defensible and tolerating it is harmless; neither is provably right
V1 1 0 5
R1 1 2 1k
R2 2 0 1k

.subckt
R1 a b 1k
.ends
.op
.end
