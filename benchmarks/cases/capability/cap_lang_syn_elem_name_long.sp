Long and unusual element names
* bench.id: cap_lang_syn_elem_name_long
* bench.group: capability
* bench.title: names like Rload_resistor_one, Vsupply_main, Cdecoupling and Lchoke_1 (first letter decides the type)
* bench.probes: v(out), i(lchoke_1)
* bench.note: the inductor is a DC short, so v(x) is 0 V and carries no information; i(lchoke_1) = 2 mA (8 V/4 k) proves that the long-named inductor is a conducting branch and not an open or a dropped card, and v(out) = 2 V needs it to be one (an open one would leave v(out) = 8 V)
* bench.tags: linear, op, syntax, element_names
Vsupply_main in 0 DC 8
Rload_resistor_one in out 3k
Rsecond.leg out x 1k
Lchoke_1 x 0 1m
Cdecoupling out 0 1n
.op
.end
