Lower-case element letters and names
* bench.id: cap_lang_syn_case_element_letters
* bench.group: capability
* bench.title: v1 / r1 / c1 / l1 in lower case form a divider with a capacitor and an inductor at DC
* bench.probes: v(out), i(l1)
* bench.note: the inductor is a DC short, so v(x) is 0 V and carries no information; i(l1) = 2 mA (8 V/4 k) proves that l1 is a conducting branch and not an open or a dropped card, and v(out) = 2 V needs it to be one (an open l1 would leave v(out) = 8 V)
* bench.tags: linear, op, syntax, case
v1 in 0 dc 8
r1 in out 3k
r2 out x 1k
l1 x 0 1m
c1 out 0 1n
.op
.end
