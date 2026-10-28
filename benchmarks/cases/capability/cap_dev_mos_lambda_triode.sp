NMOS LAMBDA applied in the triode region (Vgs=3 V, Vds sweep 0..1 V)
* bench.id: cap_dev_mos_lambda_triode
* bench.group: capability
* bench.title: NMOS LAMBDA applied in the triode region (Vgs=3 V, Vds sweep 0..1 V)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
VDS d 0 0
VG g 0 3
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.1)
.dc VDS 0 1 0.05
.end
