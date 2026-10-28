Continuation after a comment line
* bench.id: cap_lang_syn_cont_after_comment
* bench.group: capability
* bench.title: a '*' comment line between an element card and its '+' continuation does not break the card
* bench.probes: v(out)
* bench.tags: linear, op, syntax, continuation, comment
V1 in 0 DC 8
R1 in out
* this comment sits between the card and its continuation
+ 3k
R2 out 0 1k
.op
.end
