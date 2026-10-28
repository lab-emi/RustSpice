param with an empty value (x=)
* bench.id: rob_hs_syn_param_no_value
* bench.group: robustness
* bench.title: .param with an empty value (x=)
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: '.param x=' (empty value) is accepted silently (v(2)=2.5 V, exit 0); ngspice-47 aborts with 'Formula() error'
* bench.tags: hostile, syntax, parser, param, silent_risk
* bench.est_rss_mib: 64
* bench.note: x is never used by the circuit, so as written the numbers equal the valid deck (v(2)=2.5 V) and the only question is whether the malformed card is refused (low severity). The silent-number path needs a use of x: `.param x=1k`, then `.param x=`, then `R9 2 0 {x}` gives RustSpice v(2)=1.6667 V (the stale x=1k is kept, exit 0) while ngspice stops with 'Formula() error' (exit 1); a never-defined x used in a card is refused by RustSpice (ParseError, exit 1). Care: ngspice itself accepts `.param x` without an '=' (v(2)=2.5 V in both tools), so only the empty value after '=' is a safe loud_error case
.param x=
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
