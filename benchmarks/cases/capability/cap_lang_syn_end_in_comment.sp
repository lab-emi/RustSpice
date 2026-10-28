.end text inside comments
* bench.id: cap_lang_syn_end_in_comment
* bench.group: capability
* bench.title: '* .end' and '; .end' inside comments must not terminate the deck (R2 follows them)
* bench.probes: v(out)
* bench.tags: linear, op, syntax, comment
V1 in 0 DC 8
R1 in out 3k
* .end
*.end of the supply section
R3 out 0 12k ; .end of the load
R2 out 0 2k $ .end
.op
.end
