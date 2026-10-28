.ends names a different subcircuit
* bench.id: cap_lang_sub_ends_name_mismatch
* bench.group: capability
* bench.title: .subckt DIVA closed by .ends DIVB; ngspice does not check the name
* bench.probes: v(out)
* bench.tags: linear, op, subckt, syntax
.subckt DIVA in out
R1 in out 1k
R2 out 0 1k
.ends DIVB
V1 in 0 DC 8
X1 in out DIVA
.op
.end
