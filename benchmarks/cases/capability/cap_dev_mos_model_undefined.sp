NMOS referencing an undefined model
* bench.id: cap_dev_mos_model_undefined
* bench.group: capability
* bench.title: NMOS referencing an undefined model
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.tags: mosfet, model_card, dc, ngspice_error_ok
VDS d 0 0
VG g 0 2
M1 d g 0 0 NOSUCH W=10u L=1u
.dc VDS 0 5 0.1
.end
