Op, transient and DC in one deck
* bench.id: cap_lang_multi_op_tran_dc
* bench.group: capability
* bench.title: .op, .tran and .dc in one deck, with the .dc last so it cannot disturb the other two
* bench.probes: v(out)
* bench.tags: linear, multi_analysis, tran, dc, op
V1 in 0 SIN(1 1 1meg)
R1 in out 1k
R2 out 0 1k
C1 out 0 100p
.op
.tran 5n 2u
.dc V1 0 2 1
.end
