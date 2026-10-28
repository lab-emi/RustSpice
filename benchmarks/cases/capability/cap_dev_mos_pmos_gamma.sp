PMOS L1 body effect GAMMA=0.8: Id vs bulk voltage 0..3 V above the source
* bench.id: cap_dev_mos_pmos_gamma
* bench.group: capability
* bench.title: PMOS L1 body effect GAMMA=0.8: Id vs bulk voltage 0..3 V above the source
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, pmos, body_effect
VDS d 0 -3
VG g 0 -2
VB b 0 0
M1 d g 0 b PM W=20u L=1u
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u GAMMA=0.8)
.dc VB 0 3 0.25
.end
