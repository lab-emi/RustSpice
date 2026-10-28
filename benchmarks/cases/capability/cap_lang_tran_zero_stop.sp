Transient with zero stop time
* bench.id: cap_lang_tran_zero_stop
* bench.group: capability
* bench.title: .tran 1n 0 has an empty time window
* bench.probes: v(out)
* bench.expect: loud_error
* bench.tags: linear, tran, invalid_input, ngspice_error_ok
V1 in 0 PULSE(0 5 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1n
.tran 1n 0
.end
