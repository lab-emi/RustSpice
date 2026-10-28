NMOS device whose model is declared with a misspelt type NMO
* bench.id: cap_dev_mos_model_type_typo
* bench.group: capability
* bench.title: NMOS device whose model is declared with a misspelt type NMO
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.tags: mosfet, model_card, dc, ngspice_error_ok
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMO(LEVEL=1 VTO=0.7 KP=100u)
.dc VDS 0 5 0.1
.end
