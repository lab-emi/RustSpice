NMOS L1 with both KP=100u and UO=600/TOX: KP wins
* bench.id: cap_dev_mos_kp_beats_uo
* bench.group: capability
* bench.title: NMOS L1 with both KP=100u and UO=600/TOX: KP wins
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u UO=600 TOX=1e-8)
.dc VDS 0 5 0.1
.end
