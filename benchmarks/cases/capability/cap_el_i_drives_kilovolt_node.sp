Current sources forcing nodes to +10 kV and +1 kV with no voltage source in the circuit
* bench.id: cap_el_i_drives_kilovolt_node
* bench.group: capability
* bench.title: Current sources forcing nodes to +10 kV and +1 kV with no voltage source in the circuit
* bench.probes: v(a), v(b)
* bench.expect: match
* bench.oracle: dc_linear
* bench.tags: source, op, current_driven, clamp_rail_linear_anchor
* bench.note: No DC voltage source exists, so any solution-range clamp derived from the sources would be 0 V; the linear solve must reach 10 kV and 1 kV.
I1 0 a 10m
R1 a 0 1meg
I2 b 0 -1
R2 b 0 1k
.op
.end
