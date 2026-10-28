.nodeset on a circuit with one solution
* bench.id: cap_lang_nodeset_unique_solution
* bench.group: capability
* bench.title: .nodeset v(out)=3 on a linear divider must not change the unique answer
* bench.probes: v(out)
* bench.tags: linear, op, nodeset
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.nodeset v(out)=3
.op
.end
