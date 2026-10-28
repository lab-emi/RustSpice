NMOS L1 L=4u scaling
* bench.id: cap_dev_mos_length
* bench.group: capability
* bench.title: NMOS L1 L=4u scaling
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=4u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VDS 0 5 0.1
.end
