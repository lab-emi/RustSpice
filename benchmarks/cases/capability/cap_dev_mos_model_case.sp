NMOS device/model names and parameters in mixed case
* bench.id: cap_dev_mos_model_case
* bench.group: capability
* bench.title: NMOS device/model names and parameters in mixed case
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, model_card, dc, case
vds d 0 0
Vg g 0 2
m1 d g 0 0 Nm w=10U l=1U
.MODEL nM nmos(Level=1 vto=0.7 Kp=100U)
.dc VDS 0 5 0.1
.end
