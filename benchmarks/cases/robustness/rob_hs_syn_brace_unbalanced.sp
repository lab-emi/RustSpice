R value with an unclosed { expression
* bench.id: rob_hs_syn_brace_unbalanced
* bench.group: robustness
* bench.title: R value with an unclosed { expression
* bench.probes: v(1), v(2), i(v1)
* bench.expect: loud_error
* bench.tags: hostile, syntax, parser, param, silent_risk
* bench.est_rss_mib: 64
* bench.note: RustSpice trims braces without checking that they pair: `{1k`, `{{1k}` and `}1k{` are all accepted (v(2)=1.667 V, R9 read as 1k; without R9 v(2)=2.5 V) while ngspice-47 rejects them (`Closing "}" not found in line fragment`, exit 1). ngspice itself accepts a lone closing brace `1k}` (v(2)=1.667 V in both tools), so the unpaired '{' is the safe loud_error case, not any stray brace [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 {1k
.op
.end
