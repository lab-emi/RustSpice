Enhancement-load NMOS inverter (diode-connected load with body effect): VTC 0.8..5 V
* bench.id: cap_dev_ckt_nmos_enh_load_vtc
* bench.group: capability
* bench.title: Enhancement-load NMOS inverter (diode-connected load with body effect): VTC 0.8..5 V
* bench.probes: v(out)
* bench.expect: match
* bench.tags: textbook, dc, nmos, vtc, body_effect
VDD vdd 0 5
VIN in 0 0.8
M2 vdd vdd out 0 NM W=2u L=4u
M1 out in 0 0 NM W=20u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 GAMMA=0.4 PHI=0.7)
.dc VIN 0.8 5 0.1
.end
