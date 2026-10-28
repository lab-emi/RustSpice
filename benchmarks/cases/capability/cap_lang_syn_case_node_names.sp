Node names are case-insensitive
* bench.id: cap_lang_syn_case_node_names
* bench.group: capability
* bench.title: V1 IN, R1 in OUT, R2 Out: the different spellings of each node name are one node
* bench.probes: v(in), v(out)
* bench.tags: linear, op, syntax, case, node_names
* bench.note: ngspice-47 folds node names to lower case (v(out) = 8 V * 1k/(3k+1k) = 2 V); a simulator that kept case would see four separate nodes (IN, in, OUT, Out) and a floating divider
V1 IN 0 DC 8
R1 in OUT 3k
R2 Out 0 1k
.op
.end
