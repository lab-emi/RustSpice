Time values with unit letters
* bench.id: cap_lang_tran_unit_letters
* bench.group: capability
* bench.title: .tran 10ns 5us (scale factor followed by unit letters) on an RC step
* bench.probes: v(out)
* bench.tags: linear, tran, syntax
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10ns 5us
.end
