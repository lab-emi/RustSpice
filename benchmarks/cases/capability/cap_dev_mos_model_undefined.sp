NMOS referencing an undefined model
* bench.id: cap_dev_mos_model_undefined
* bench.group: capability
* bench.title: NMOS referencing an undefined model
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: undefined model silently replaced by the default MOS model, numbers are returned with exit 0 and empty stderr: |i(vds)| = 4.0e-4 A at vds = 5 V (with NOSUCH NMOS(VTO=0.7 KP=100u) defined: 8.45e-4 A); ngspice-47 aborts ("can't find model 'nosuch'", exit 1)
* bench.tags: mosfet, model_card, dc, ngspice_error_ok
VDS d 0 0
VG g 0 2
M1 d g 0 0 NOSUCH W=10u L=1u
.dc VDS 0 5 0.1
.end
