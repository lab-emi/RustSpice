NMOS 1:2 current mirror, IREF=100 uA, output voltage sweep 0..5 V
* bench.id: cap_dev_ckt_current_mirror_sweep
* bench.group: capability
* bench.title: NMOS 1:2 current mirror, IREF=100 uA, output voltage sweep 0..5 V
* bench.probes: i(vo), v(g)
* bench.expect: match
* bench.tags: textbook, dc, nmos, mirror
VDD vdd 0 5
IREF vdd g 100u
M1 g g 0 0 NM W=20u L=2u
M2 out g 0 0 NM W=40u L=2u
VO out 0 0
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VO 0 5 0.1
.end
