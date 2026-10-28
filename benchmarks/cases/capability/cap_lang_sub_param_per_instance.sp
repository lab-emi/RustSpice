Different overrides on different instances
* bench.id: cap_lang_sub_param_per_instance
* bench.group: capability
* bench.title: three instances of DIV with rv = default (1k), 3k and 7k must not share parameter values
* bench.probes: v(o1), v(o2), v(o3)
* bench.tags: linear, op, subckt, param
* bench.note: ngspice-47 computes v(o1) = 8 V * 1k/2k = 4 V, v(o2) = 8 V * 1k/4k = 2 V, v(o3) = 8 V * 1k/8k = 1 V (hand-checked)
.subckt DIV in out PARAMS: rv=1k
R1 in out {rv}
R2 out 0 1k
.ends DIV
V1 in 0 DC 8
X1 in o1 DIV
X2 in o2 DIV rv=3k
X3 in o3 DIV rv=7k
.op
.end
