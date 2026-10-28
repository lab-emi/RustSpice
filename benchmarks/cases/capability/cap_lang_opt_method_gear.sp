Integration method option
* bench.id: cap_lang_opt_method_gear
* bench.group: capability
* bench.title: .options method=gear on an RC step (tau = 1 us) with 100 steps per tau
* bench.probes: v(out)
* bench.tags: linear, tran, options
.options method=gear
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 10n 5u
.end
