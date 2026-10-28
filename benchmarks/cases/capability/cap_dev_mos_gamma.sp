NMOS L1 body effect GAMMA=0.8 (PHI default): Id vs Vbs
* bench.id: cap_dev_mos_gamma
* bench.group: capability
* bench.title: NMOS L1 body effect GAMMA=0.8 (PHI default): Id vs Vbs
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 3
VG g 0 2
VB b 0 0
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u GAMMA=0.8)
.dc VB -3 0 0.25
.end
