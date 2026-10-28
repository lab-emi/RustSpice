Probe card
* bench.id: cap_lang_dir_probe
* bench.group: capability
* bench.title: .probe v(out) (HSPICE-style output request) next to .op
* bench.probes: v(out)
* bench.tags: linear, op, output_card
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.probe v(out)
.op
.end
