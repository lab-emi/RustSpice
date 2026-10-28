Source follower with body effect (GAMMA=0.5), Vg sweep 0.8..3.3 V
* bench.id: cap_dev_ckt_src_follower_body
* bench.group: capability
* bench.title: Source follower with body effect (GAMMA=0.5), Vg sweep 0.8..3.3 V
* bench.probes: v(s), i(vdd)
* bench.tol.abs: 1e-7
* bench.expect: match
* bench.tags: textbook, dc, nmos, body_effect
* bench.note: abs 1e-7: near cutoff the high-impedance source node differs by ~1e-8 V (RustSpice drain-source gmin leakage, absent in ngspice)
VDD vdd 0 3.3
VG g 0 0.8
M1 vdd g s 0 NM W=20u L=1u
RS s 0 10k
.model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.02 GAMMA=0.5 PHI=0.7)
.dc VG 0.8 3.3 0.1
.end
