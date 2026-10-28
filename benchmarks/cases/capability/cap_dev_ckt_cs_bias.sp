Common-source bias: divider 150k/100k, RS=2k, RD=4k, VDD=10 V (hand-solved)
* bench.id: cap_dev_ckt_cs_bias
* bench.group: capability
* bench.title: Common-source bias: divider 150k/100k, RS=2k, RD=4k, VDD=10 V (hand-solved)
* bench.probes: v(g), v(d), v(s), i(vdd)
* bench.expect: match
* bench.tags: textbook, op, nmos, hand_checked
* bench.note: lambda=0: Vov solves Vov^2+Vov-3=0 -> Vov=1.3028, Id=0.8486 mA, Vs=1.6972 V, Vd=6.6056 V (saturation)
VDD vdd 0 10
RG1 vdd g 150k
RG2 g 0 100k
RD vdd d 4k
RS s 0 2k
M1 d g s 0 NM W=20u L=2u
.model NM NMOS(LEVEL=1 VTO=1 KP=100u)
.op
.end
