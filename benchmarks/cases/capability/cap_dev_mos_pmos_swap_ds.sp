PMOS L1 reverse operation: Vds swept -3..3 V, bulk at +3.5 V
* bench.id: cap_dev_mos_pmos_swap_ds
* bench.group: capability
* bench.title: PMOS L1 reverse operation: Vds swept -3..3 V, bulk at +3.5 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, pmos
VDS d 0 0
VG g 0 -2
VB b 0 3.5
M1 d g 0 b PM W=20u L=1u
.model PM PMOS(LEVEL=1 VTO=-0.7 KP=50u LAMBDA=0.02)
.dc VDS -3 3 0.1
.end
