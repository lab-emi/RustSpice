NMOS differential pair, 100 uA tail source, 20k loads, 50 mV offset
* bench.id: cap_dev_ckt_diffpair_op
* bench.group: capability
* bench.title: NMOS differential pair, 100 uA tail source, 20k loads, 50 mV offset
* bench.probes: v(d1), v(d2), v(t)
* bench.expect: match
* bench.tags: textbook, op, nmos, diffpair
VDD vdd 0 5
IT t 0 100u
M1 d1 g1 t 0 NM W=20u L=2u
M2 d2 g2 t 0 NM W=20u L=2u
RD1 vdd d1 20k
RD2 vdd d2 20k
VG1 g1 0 2.55
VG2 g2 0 2.5
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.op
.end
