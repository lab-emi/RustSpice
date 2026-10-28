Inductor multiplier m=2 in an RL step
* bench.id: cap_el_l_m_multiplier
* bench.group: capability
* bench.title: Inductor multiplier m=2 in an RL step
* bench.probes: v(out)
* bench.expect: match
* bench.tags: passive, tran, multiplier, silent_risk
* bench.note: ngspice divides the inductance by m (parallel inductors): tau = 0.5 us.
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in out 1
L1 out 0 1u m=2
.tran 20n 5u
.end
