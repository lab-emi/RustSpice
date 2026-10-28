NMOS L1 sharp threshold: Id-Vgs 0.5..0.9 V around VTO=0.7 V
* bench.id: cap_dev_mos_subthreshold
* bench.group: capability
* bench.title: NMOS L1 sharp threshold: Id-Vgs 0.5..0.9 V around VTO=0.7 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 3
VG g 0 0.5
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VG 0.5 0.9 0.01
.end
