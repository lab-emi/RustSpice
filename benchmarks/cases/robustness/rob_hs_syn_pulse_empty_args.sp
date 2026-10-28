PULSE() without arguments
* bench.id: rob_hs_syn_pulse_empty_args
* bench.group: robustness
* bench.title: PULSE() without arguments
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: PULSE() (and PULSE(1)) is accepted with exit 0 and becomes a DC 0 V source; ngspice-47 rejects the card (exit 1, 'parameter value out of range or the wrong type'); PULSE(1 2) is accepted by both
* bench.tags: hostile, syntax, parser, source, silent_risk
* bench.est_rss_mib: 64
* bench.note: Argument validation, not parsing: a valid PULSE is read correctly by RustSpice, so the gap closes only when too few PULSE arguments are refused
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
V9 3 0 PULSE()
R9 3 0 1k
.op
.end
