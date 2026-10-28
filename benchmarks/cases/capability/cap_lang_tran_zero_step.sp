Transient with zero step
* bench.id: cap_lang_tran_zero_step
* bench.group: capability
* bench.title: .tran 0 1u has a zero print step
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, tran, invalid_input, ngspice_error_ok
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 0 1u
.end
