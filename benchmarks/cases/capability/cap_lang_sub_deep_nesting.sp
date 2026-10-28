Deep but finite subcircuit nesting
* bench.id: cap_lang_sub_deep_nesting
* bench.group: capability
* bench.title: 16 levels of .subckt, each instantiating the next level, ending in one resistor
* bench.probes: v(out)
* bench.tags: linear, op, subckt, hierarchy, depth
* bench.note: ngspice-47 itself crashes or reports bogus recursion at some depths (20 and >= 23 observed), so the depth is kept at 16
.subckt L16 a b
R1 a b 1k
.ends L16
.subckt L15 a b
X1 a b L16
.ends L15
.subckt L14 a b
X1 a b L15
.ends L14
.subckt L13 a b
X1 a b L14
.ends L13
.subckt L12 a b
X1 a b L13
.ends L12
.subckt L11 a b
X1 a b L12
.ends L11
.subckt L10 a b
X1 a b L11
.ends L10
.subckt L9 a b
X1 a b L10
.ends L9
.subckt L8 a b
X1 a b L9
.ends L8
.subckt L7 a b
X1 a b L8
.ends L7
.subckt L6 a b
X1 a b L7
.ends L6
.subckt L5 a b
X1 a b L6
.ends L5
.subckt L4 a b
X1 a b L5
.ends L4
.subckt L3 a b
X1 a b L4
.ends L3
.subckt L2 a b
X1 a b L3
.ends L2
.subckt L1 a b
X1 a b L2
.ends L1
V1 in 0 DC 8
X1 in out L1
R9 out 0 3k
.op
.end
