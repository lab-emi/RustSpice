NMOS L1 VTO=1.5 V: Id-Vgs threshold
* bench.id: cap_dev_mos_vto
* bench.group: capability
* bench.title: NMOS L1 VTO=1.5 V: Id-Vgs threshold
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 3
VG g 0 0
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=1.5 KP=100u)
.dc VG 0 4 0.1
.end
