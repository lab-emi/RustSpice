Node attached to a single element
* bench.id: cap_lang_syn_floating_single_element
* bench.group: capability
* bench.title: R3 hangs off the divider into a node with no other connection; it carries no current
* bench.probes: v(out), v(dangle)
* bench.tags: linear, op, topology, floating
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
R3 out dangle 1k
.op
.end
