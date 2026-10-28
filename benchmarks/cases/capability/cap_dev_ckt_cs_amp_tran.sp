NMOS common-source amplifier, 10k load with 1 pF, 5 MHz sine on 1.2 V bias
* bench.id: cap_dev_ckt_cs_amp_tran
* bench.group: capability
* bench.title: NMOS common-source amplifier, 10k load with 1 pF, 5 MHz sine on 1.2 V bias
* bench.probes: v(d)
* bench.expect: match
* bench.tags: textbook, tran, nmos, amplifier
VDD vdd 0 5
RD vdd d 10k
CL d 0 1p
VIN g 0 SIN(1.2 0.15 5Meg)
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.tran 1n 600n
.end
