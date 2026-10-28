NMOS inverter with 10 kohm load: VTC 0..5 V
* bench.id: cap_dev_ckt_nmos_resload_vtc
* bench.group: capability
* bench.title: NMOS inverter with 10 kohm load: VTC 0..5 V
* bench.probes: v(out)
* bench.expect: match
* bench.tags: textbook, dc, nmos, vtc
VDD vdd 0 5
RD vdd out 10k
VIN in 0 0
M1 out in 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VIN 0 5 0.1
.end
