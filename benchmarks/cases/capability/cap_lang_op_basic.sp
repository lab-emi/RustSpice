Resistor divider operating point
* bench.id: cap_lang_op_basic
* bench.group: capability
* bench.title: .op on a 1:3 resistor divider with a current-probe source
* bench.probes: v(in), v(out), i(v1)
* bench.oracle: dc_linear
* bench.tags: linear, op, anchor
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
