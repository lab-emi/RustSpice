Capacitor tc1 with .temp 77
* bench.id: cap_el_c_tc1_temp
* bench.group: capability
* bench.title: Capacitor tc1 with .temp 77
* bench.probes: v(out)
* bench.expect: match
* bench.tags: passive, tran, temperature, silent_risk
* bench.note: Exact: C = 1u*(1+0.01*50) = 1.5 uF.
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1u tc1=0.01
.temp 77
.tran 20u 5m
.end
