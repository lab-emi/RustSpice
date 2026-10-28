Plot card
* bench.id: cap_lang_dir_plot
* bench.group: capability
* bench.title: .plot dc v(out) next to a valid .dc is accepted
* bench.probes: v(out)
* bench.tags: linear, dc, output_card
V1 in 0 0
R1 in out 1k
R2 out 0 3k
.dc V1 0 5 1
.plot dc v(out)
.end
