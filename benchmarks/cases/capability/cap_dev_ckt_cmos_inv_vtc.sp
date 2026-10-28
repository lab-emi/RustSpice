CMOS inverter VTC, VDD=3 V
* bench.id: cap_dev_ckt_cmos_inv_vtc
* bench.group: capability
* bench.title: CMOS inverter VTC, VDD=3 V
* bench.probes: v(out), i(vdd)
* bench.expect: match
* bench.tags: textbook, dc, cmos, vtc
VDD vdd 0 3
VIN in 0 0
MP out in vdd vdd PM W=20u L=1u
MN out in 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
.dc VIN 0 3 0.05
.end
