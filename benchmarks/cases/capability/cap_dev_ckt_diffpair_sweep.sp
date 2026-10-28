NMOS differential pair transfer curve, Vg1 swept 2.2..2.8 V
* bench.id: cap_dev_ckt_diffpair_sweep
* bench.group: capability
* bench.title: NMOS differential pair transfer curve, Vg1 swept 2.2..2.8 V
* bench.probes: v(d1), v(d2), v(t)
* bench.expect: match
* bench.tags: textbook, dc, nmos, diffpair
VDD vdd 0 5
IT t 0 100u
M1 d1 g1 t 0 NM W=20u L=2u
M2 d2 g2 t 0 NM W=20u L=2u
RD1 vdd d1 20k
RD2 vdd d2 20k
VG1 g1 0 2.2
VG2 g2 0 2.5
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VG1 2.2 2.8 0.02
.end
