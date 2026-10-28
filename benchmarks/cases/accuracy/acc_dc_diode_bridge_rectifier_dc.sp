Four-diode bridge driven by 10 V DC into a 1k load: two diodes conduct, two are reverse biased by 9.3 V
* bench.id: acc_dc_diode_bridge_rectifier_dc
* bench.group: accuracy
* bench.title: Four-diode bridge driven by 10 V DC into a 1k load: two diodes conduct, two are reverse biased by 9.3 V
* bench.probes: v(a), v(b), i(v1)
* bench.tol.abs: 1e-12
* bench.tags: nonlinear, op, diode, bridge
* bench.note: tol.abs tightened to 1e-12 A (the default 1e-9 A would be 1e-5 relative on mA currents); no dc_nonlinear oracle (reverse-biased diodes follow the SPICE3 reverse law)
V1 p 0 10
D1 p a dm
D2 0 a dm
D3 b p dm
D4 b 0 dm
Rl a b 1k
.model dm D(IS=1e-14 N=1)
.op
.end
