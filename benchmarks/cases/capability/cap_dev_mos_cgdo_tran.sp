NMOS CGDO=1 nF/m gate-drain overlap (Miller) capacitance, W=100u
* bench.id: cap_dev_mos_cgdo_tran
* bench.group: capability
* bench.title: NMOS CGDO=1 nF/m gate-drain overlap (Miller) capacitance, W=100u
* bench.probes: v(g), v(d)
* bench.expect: match
* bench.tags: mosfet, level1, tran, capacitance
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=100u L=1u

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u CGDO=1n)
.tran 0.05n 40n
.end
