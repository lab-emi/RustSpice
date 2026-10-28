Global node declared with .global
* bench.id: cap_lang_sub_global_node
* bench.group: capability
* bench.title: .global vdd makes 'vdd' visible inside .subckt without being a port
* bench.probes: v(out), v(vdd)
* bench.tags: linear, op, subckt, global
.global vdd
.subckt PULL out
R1 vdd out 1k
.ends PULL
V1 vdd 0 DC 5
X1 out PULL
R2 out 0 3k
.op
.end
