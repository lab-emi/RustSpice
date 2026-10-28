Instance of an undefined subcircuit
* bench.id: cap_lang_sub_undefined
* bench.group: capability
* bench.title: X1 ... NOSUCH refers to a subcircuit that is never defined
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, subckt, invalid_input, ngspice_error_ok
V1 in 0 DC 8
R1 in out 1k
X1 out NOSUCH
.op
.end
