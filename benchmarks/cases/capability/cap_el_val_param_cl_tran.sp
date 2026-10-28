Values from .param in C, L, R and a PULSE amplitude (tau = 1 ms)
* bench.id: cap_el_val_param_cl_tran
* bench.group: capability
* bench.title: Values from .param in C, L, R and a PULSE amplitude (tau = 1 ms)
* bench.probes: v(c1), v(l1n)
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: parser, passive, tran, param
* bench.note: RC: tau = 1k*1u = 1 ms; RL: tau = 1 H / 1k = 1 ms.
.param vdd=1 cv=1u lv=1 rr=1k
V1 in 0 PULSE(0 {vdd} 0 1n 1n 1 2)
R1 in c1 {rr}
C1 c1 0 {cv}
R2 in l1n {rr}
L1 l1n 0 {lv}
.tran 20u 5m
.end
