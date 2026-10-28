NMOS L1 reverse operation: Vds swept -3..3 V (source/drain swap), bulk at -3.5 V
* bench.id: cap_dev_mos_swap_ds
* bench.group: capability
* bench.title: NMOS L1 reverse operation: Vds swept -3..3 V (source/drain swap), bulk at -3.5 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 0
VG g 0 2
VB b 0 -3.5
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VDS -3 3 0.1
.end
