Capacitor multiplier m=2 in an RC step
* bench.id: cap_el_c_m_multiplier
* bench.group: capability
* bench.title: Capacitor multiplier m=2 in an RC step
* bench.probes: v(out)
* bench.expect: match
* bench.tags: passive, tran, multiplier, silent_risk
* bench.note: Exact: C_eff = 2 uF, tau = 2 ms.
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in out 1k
C1 out 0 1u m=2
.tran 20u 5m
.end
