Continuation lines in header, instance and param cards
* bench.id: cap_lang_syn_cont_subckt_header
* bench.group: capability
* bench.title: a .param card, the .subckt header and the X instance are each split over '+' lines; the subcircuit is closed by a bare .ends
* bench.probes: v(out)
* bench.tags: linear, op, syntax, continuation, subckt, param
* bench.note: ngspice-47 joins the lines (v(out) = 8 V * 1k/(3k+1k) = 2 V); a lost continuation would drop a port, a parameter or the subcircuit name
.param ra=3k
+ rb=1k
.subckt DIV
+ in out
R1 in out {ra}
R2 out 0 {rb}
.ends
V1 in 0 DC 8
X1
+ in out
+ DIV
.op
.end
