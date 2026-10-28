.subckt title_line_must_be_ignored a b
* bench.id: rob_hs_ff_title_line_dot_card
* bench.group: robustness
* bench.title: first line looks like a .subckt card: it is the title and must be ignored
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.known_gap: title-line-parsed-as-card
* bench.gap.note: the '.subckt title_line_must_be_ignored a b' first line is parsed as a card and swallows the whole deck, so RustSpice refuses with 'no non-ground nodes' (ngspice solves: v(2)=2.5 V)
* bench.tags: file, parser, silent_risk
* bench.est_rss_mib: 64
* bench.note: line 1 is always the title in ngspice-47 (v(2)=2.5 V, equal to the 5 V divider closed form); RustSpice parses a first line that starts with '.' as a card. Minimal variants: a plain-text first line works in both tools, '.subckt ...' swallows the whole deck, '.op' as line 1 runs the OP twice (two plots) Same root as cap_lang_syn_blank_first_line (title = first non-blank line not starting with '.', '*' or ';'); a first line '* c' or '; c' is harmless in RustSpice (verified).
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
.op
.end
