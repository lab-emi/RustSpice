Save card restricts the saved vectors
* bench.id: cap_lang_dir_save
* bench.group: capability
* bench.title: .save v(out) restricts ngspice's output to one vector; v(out) must still be present and exact
* bench.probes: v(out)
* bench.tags: linear, op, output_card
V1 in 0 DC 8
R1 in out 3k
R2 out 0 1k
.save v(out)
.op
.end
