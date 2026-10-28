R card with trailing junk tokens after the value
* bench.id: rob_hs_syn_r_extra_tokens
* bench.group: robustness
* bench.title: R card with trailing junk tokens after the value
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: the two trailing tokens after the value are dropped silently and R9 (1k) is stamped (v(2)=1.667 V, exit 0); ngspice-47 rejects the card with 'unknown parameter (1k)'
* bench.tags: hostile, syntax, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: the refusal threshold is ngspice-specific: ngspice-47 tolerates exactly ONE extra token (`R9 2 0 1k 1k` gives v(2)=1.6667 V in both tools), so this deck with two extra tokens sits at its boundary; `1k foo=1` and `1k junk` are rejected by ngspice and dropped silently by RustSpice (v(2)=1.6667 V). RustSpice never examines the tokens a card does not consume. The valid `m=2` goes through the same code path: ngspice 1.25 V (R9 doubled), RustSpice 1.6667 V (silent-ignore-multiplier, valid input dropped)
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 1k 1k 1k
.op
.end
