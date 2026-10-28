PMOS L1 with VTO=+0.7 (positive sign for a p-channel device)
* bench.id: cap_dev_mos_pmos_vto_positive
* bench.group: capability
* bench.title: PMOS L1 with VTO=+0.7 (positive sign for a p-channel device)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, pmos, model_card
VDS d 0 0
VG g 0 -2
M1 d g 0 0 PM W=20u L=1u
.model PM PMOS(LEVEL=1 VTO=0.7 KP=50u LAMBDA=0.02)
.dc VDS 0 -5 -0.1
.end
