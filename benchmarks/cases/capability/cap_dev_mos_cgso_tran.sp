NMOS CGSO=1 nF/m gate-source overlap capacitance (W=100u -> 100 fF)
* bench.id: cap_dev_mos_cgso_tran
* bench.group: capability
* bench.title: NMOS CGSO=1 nF/m gate-source overlap capacitance (W=100u -> 100 fF)
* bench.probes: v(g), v(d)
* bench.expect: match
* bench.tags: mosfet, level1, tran, capacitance
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=100u L=1u

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u CGSO=1n)
.tran 0.05n 40n
.end
