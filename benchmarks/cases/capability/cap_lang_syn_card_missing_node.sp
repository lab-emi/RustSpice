Resistor card with a single node
* bench.id: cap_lang_syn_card_missing_node
* bench.group: capability
* bench.title: R1 in 1k (one node and a value) is not a valid instance line; ngspice warns and drops it
* bench.probes: v(in)
* bench.expect: either
* bench.tags: linear, op, syntax, invalid_input
* bench.note: genuinely ambiguous, no verdict is possible: ngspice-47 warns 'not a valid resistor instance line, ignored' and simulates a different circuit without R1 (v(in) = 8 V), RustSpice refuses ('Resistor requires R<name> <n+> <n-> <value>', exit 1); dropping the line with a warning and refusing are both defensible, and the ngspice numbers are those of a circuit the author did not write, so they are no reference; kept as info rather than loud_error so that a warn-and-continue RustSpice is not misreported as a silent failure
V1 in 0 DC 8
R1 in 1k
R2 in 0 1k
.op
.end
