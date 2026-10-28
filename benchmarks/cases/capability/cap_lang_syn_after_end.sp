Cards after .end
* bench.id: cap_lang_syn_after_end
* bench.group: capability
* bench.title: R9 (1 ohm) follows .end; ngspice-47 still reads it, so v(out) collapses to about 2.7 mV
* bench.probes: v(out)
* bench.tags: linear, op, syntax, termination
* bench.note: classic SPICE stops at .end; ngspice-47 and RustSpice both keep reading, so this pins parity with the reference
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.op
.end
R9 out 0 1
