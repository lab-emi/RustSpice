NMOS L1 Id-Vgs at Vds=3 V (cutoff to saturation)
* bench.id: cap_dev_mos_idvgs
* bench.group: capability
* bench.title: NMOS L1 Id-Vgs at Vds=3 V (cutoff to saturation)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 3
VG g 0 0
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VG 0 3 0.05
.end
