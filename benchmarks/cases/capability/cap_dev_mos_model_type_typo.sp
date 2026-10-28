NMOS device whose model is declared with a misspelt type NMO
* bench.id: cap_dev_mos_model_type_typo
* bench.group: capability
* bench.title: NMOS device whose model is declared with a misspelt type NMO
* bench.probes: i(vds)
* bench.expect: loud_error
* bench.known_gap: silent-ignore-directive
* bench.gap.note: misspelt model type NMO silently ignored (exit 0, empty stderr): the MOS runs with the default model, |i(vds)| = 4.0e-4 A at vds = 5 V instead of 8.45e-4 A (type corrected to NMOS: RustSpice = closed form 0.5*100u*10*(2-0.7)^2); ngspice-47 aborts ('model type mismatch', exit 1)
* bench.tags: mosfet, model_card, dc, ngspice_error_ok
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMO(LEVEL=1 VTO=0.7 KP=100u)
.dc VDS 0 5 0.1
.end
