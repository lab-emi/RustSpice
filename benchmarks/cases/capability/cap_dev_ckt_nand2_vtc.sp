CMOS NAND2 with B=VDD: A sweep (series NMOS with body effect)
* bench.id: cap_dev_ckt_nand2_vtc
* bench.group: capability
* bench.title: CMOS NAND2 with B=VDD: A sweep (series NMOS with body effect)
* bench.probes: v(out), v(x)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: textbook, dc, cmos, nand2, body_effect
* bench.note: abs 1e-8: with both inputs low the floating node x sits at 6.5e-10 V in RustSpice (gmin across drain-source) and 0 V in ngspice
VDD vdd 0 3
VA a 0 0
VB b 0 3
MP1 out a vdd vdd PM W=20u L=1u
MP2 out b vdd vdd PM W=20u L=1u
MN1 out a x 0 NM W=20u L=1u
MN2 x b 0 0 NM W=20u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 GAMMA=0.4 PHI=0.7)
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
.dc VA 0 3 0.05
.end
