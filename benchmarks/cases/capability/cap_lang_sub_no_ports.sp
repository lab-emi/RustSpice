Subcircuit with no ports
* bench.id: cap_lang_sub_no_ports
* bench.group: capability
* bench.title: .subckt CELL with zero ports is a self-contained source and divider
* bench.probes: v(x1.a), v(x1.b)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: .subckt with zero ports: ".subckt requires a name and at least one port"
* bench.tags: linear, op, subckt, syntax
* bench.note: ngspice-47 computes v(x1.a) = 4 V and v(x1.b) = 4 V * 3k/(1k+3k) = 3 V (hand-checked)
.subckt CELL
V1 a 0 DC 4
R1 a b 1k
R2 b 0 3k
.ends CELL
X1 CELL
.op
.end
