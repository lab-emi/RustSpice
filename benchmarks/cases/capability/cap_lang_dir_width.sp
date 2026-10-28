Width card
* bench.id: cap_lang_dir_width
* bench.group: capability
* bench.title: .width out=80 (print width) next to .op
* bench.probes: v(out)
* bench.tags: linear, op, output_card
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.width out=80
.op
.end
