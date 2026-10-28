inline ; comment after an element
* bench.id: rob_hs_syn_inline_comment_semicolon
* bench.group: robustness
* bench.title: inline ; comment after an element
* bench.probes: v(1), v(2), i(v1)
* bench.expect: match
* bench.tags: syntax, parser, lenient_ok
* bench.est_rss_mib: 64
* bench.note: the comment after the X9 card matters: the subcircuit name is the last token of an X card, so a comment that is not stripped would be read as the subcircuit name and refused (v(2)=1.25 V when handled). The R9 line alone could not tell a stripped comment from ignored trailing tokens. Strengthened in calibration
.subckt rr a b
R1 a b 1k
.ends
V1 1 0 5
R1 1 2 1k
R2 2 0 1k
R9 2 0 1k ; trailing comment
X9 2 0 rr ; trailing comment 7
.op
.end
