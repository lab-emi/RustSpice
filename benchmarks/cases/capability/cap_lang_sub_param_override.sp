Instance parameter override
* bench.id: cap_lang_sub_param_override
* bench.group: capability
* bench.title: X1 ... DIV rv=3k overrides the PARAMS: default rv=1k
* bench.probes: v(out)
* bench.known_gap: unsupported-syntax-loud
* bench.gap.note: loud parse error: X1 ... DIV rv=3k: "Unknown subcircuit `rv=3k`"
* bench.tags: linear, op, subckt, param
* bench.note: ngspice-47 computes v(out) = 8 V * 1k/(3k+1k) = 2 V with the override rv = 3k (the 1k default would give 4 V; hand-checked)
.subckt DIV in out PARAMS: rv=1k
R1 in out {rv}
R2 out 0 1k
.ends DIV
V1 in 0 DC 8
X1 in out DIV rv=3k
.op
.end
