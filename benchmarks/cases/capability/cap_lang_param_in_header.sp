Parameter in a subcircuit header
* bench.id: cap_lang_param_in_header
* bench.group: capability
* bench.title: .subckt DIV in out PARAMS: rv=1k uses the default when X1 gives no override
* bench.probes: v(out)
* bench.tags: linear, op, param, subckt
* bench.note: ngspice-47 computes v(out) = 6 V * 1k/(1k+1k) = 3 V with the default rv = 1k (hand-checked)
.subckt DIV in out PARAMS: rv=1k
R1 in out {rv}
R2 out 0 1k
.ends DIV
V1 in 0 DC 6
X1 in out DIV
.op
.end
