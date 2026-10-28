unknown dot-card
* bench.id: rob_hs_syn_unknown_dot_card
* bench.group: robustness
* bench.title: unknown dot-card
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the unknown card '.frobnicate 1 2 3' is accepted without any diagnostic (exit 0, empty stderr, v(2) = 2.5 V, identical to the deck without the card); ngspice-47 aborts with "unimplemented dot command '.frobnicate'" (exit 1)
* bench.tags: hostile, syntax, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: slug lenient-malformed-input (SUITE_SPEC 13.1): a card that carries no semantics cannot be 'honoured', so the defect is the acceptance of input ngspice rejects, the same mechanism and input class as cap_lang_dir_unknown_card (.foobar)
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.frobnicate 1 2 3
.op
.end
