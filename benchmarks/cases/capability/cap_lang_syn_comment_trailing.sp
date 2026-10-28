Trailing comments with semicolon and dollar
* bench.id: cap_lang_syn_comment_trailing
* bench.group: capability
* bench.title: R1 ... 3k ; note and R2 ... 1k $ note end-of-line comments
* bench.probes: v(out)
* bench.tags: linear, op, syntax, comment
V1 in 0 DC 8 ; supply
R1 in out 3k ; upper leg
R2 out 0 1k $ lower leg
.op
.end
