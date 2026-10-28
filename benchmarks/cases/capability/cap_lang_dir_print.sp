Print card
* bench.id: cap_lang_dir_print
* bench.group: capability
* bench.title: .print tran v(out) next to a valid .tran is accepted
* bench.probes: v(out)
* bench.tags: linear, tran, output_card
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u
.print tran v(out) v(in)
.end
