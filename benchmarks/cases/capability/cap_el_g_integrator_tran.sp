VCCS into a capacitor (integrator with 1 Mohm bleed)
* bench.id: cap_el_g_integrator_tran
* bench.group: capability
* bench.title: VCCS into a capacitor (integrator with 1 Mohm bleed)
* bench.probes: v(out)
* bench.expect: match
* bench.oracle: tran_lti
* bench.tags: controlled, tran, vccs, linear
* bench.note: dv/dt = gm*vin/C = 1e6 V/s: a 5 V ramp over 5 us (the bleed, tau 1 ms, bends it by 0.25 %).
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
G1 0 out in 0 1m
C1 out 0 1n
R1 out 0 1meg
.tran 50n 5u
.end
