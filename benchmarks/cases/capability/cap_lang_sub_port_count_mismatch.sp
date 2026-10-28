Instance with the wrong number of nodes
* bench.id: cap_lang_sub_port_count_mismatch
* bench.group: capability
* bench.title: DIV declares 2 ports but X1 connects 3 nodes
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, subckt, invalid_input, ngspice_error_ok
.subckt DIV in out
R1 in out 1k
R2 out 0 1k
.ends DIV
V1 in 0 DC 8
X1 in out extra DIV
.op
.end
