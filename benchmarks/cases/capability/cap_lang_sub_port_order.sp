Ports connected in a different order than declared
* bench.id: cap_lang_sub_port_order
* bench.group: capability
* bench.title: X1 binds ports (top mid bot) to nodes (n3 n1 n2): asymmetric legs expose any port permutation
* bench.probes: v(n1), v(n2), v(n3)
* bench.tags: linear, op, subckt
.subckt LEG top mid bot
R1 top mid 1k
R2 mid bot 3k
.ends LEG
V1 n3 0 DC 8
V2 n2 0 DC 2
X1 n3 n1 n2 LEG
R3 n1 0 2k
.op
.end
