Subcircuit defined inside another subcircuit
* bench.id: cap_lang_sub_nested_definition
* bench.group: capability
* bench.title: .subckt INNER defined inside .subckt OUTER and used only there
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: .subckt inside .subckt: "Unknown subcircuit `inner`"
* bench.tags: linear, op, subckt, hierarchy
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(1k+1k) = 4 V (hand-checked)
.subckt OUTER in out
.subckt INNER a b
R1 a b 1k
R2 b 0 1k
.ends INNER
X1 in out INNER
.ends OUTER
V1 in 0 DC 8
X9 in out OUTER
.op
.end
