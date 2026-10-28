PMOS L1 Id-Vds at Vgs=-2 V
* bench.id: cap_dev_mos_pmos_idvds
* bench.group: capability
* bench.title: PMOS L1 Id-Vds at Vgs=-2 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, pmos
VDS d 0 0
VG g 0 -2
M1 d g 0 0 PM W=20u L=1u
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
.dc VDS 0 -5 -0.1
.end
