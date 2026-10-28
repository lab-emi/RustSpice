Element name with a letter no device uses
* bench.id: cap_lang_syn_elem_unknown_letter
* bench.group: capability
* bench.title: _R1 starts with an underscore, which is not a device letter
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, op, syntax, element_names, invalid_input, ngspice_error_ok
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
_R3 out 0 1k
.op
.end
