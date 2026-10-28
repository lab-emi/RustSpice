R value with an unterminated quote
* bench.id: rob_hs_syn_quote_unbalanced
* bench.group: robustness
* bench.title: R value with an unterminated quote
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.known_gap: lenient-malformed-input
* bench.gap.note: an unpaired quote is stripped and the rest is read as 1k (v(2)=1.667 V, exit 0); ngspice-47 aborts with a netlist error. The balanced '1k' variant agrees in both tools
* bench.tags: hostile, syntax, parser, param, silent_risk
* bench.est_rss_mib: 64
* bench.note: both `'1k` and `1k'` are accepted by RustSpice (the quote characters are trimmed without checking that they pair, R9 read as 1k, v(2)=1.6667 V) and rejected by ngspice-47 (`Closing "}" not found in line fragment`, exit 1). The balanced `'1k'` gives v(2)=1.6667 V in both tools, closed form R2||R9 = 500 ohm: 5*500/1500 = 1.6667 V
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 '1k
.op
.end
