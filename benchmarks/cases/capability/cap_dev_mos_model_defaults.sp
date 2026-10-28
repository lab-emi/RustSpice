NMOS .model with no parameters (VTO=0, KP=20u, all defaults), Vgs=2 V
* bench.id: cap_dev_mos_model_defaults
* bench.group: capability
* bench.title: NMOS .model with no parameters (VTO=0, KP=20u, all defaults), Vgs=2 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, model_card, dc
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS
.dc VDS 0 5 0.1
.end
