NMOS L1 model TNOM=60 (simulated at 27 C)
* bench.id: cap_dev_mos_tnom_model
* bench.group: capability
* bench.title: NMOS L1 model TNOM=60 (simulated at 27 C)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, temperature, model_param
* bench.note: evidence: RustSpice output is bit-identical with and without model TNOM=60, with .options tnom=60 instead of the model TNOM and with .temp 100, and equals ngspice with T == TNOM (1e-14 A); tight ngspice moves by 4.0e-4 A (12% of its 3.2 mA maximum). Independent oracle: level-1 with the textbook temperature scaling (KP*(T/Tnom)^-1.5, Varshni Eg, ni ~ T^1.5*exp(-Eg/2kT), VTO shift 0.5*dEg + 0.5*dPHI, T = 300.15 K, Tnom = 333.15 K) gives VTO = 0.72663 V and KP = 1.1694e-4 A/V^2 and reproduces tight ngspice to 9.4e-7 relative while RustSpice equals the unscaled solution (5.7e-7). Slug: the mechanism is the missing temperature model (RustSpice never leaves T = TNOM), not a lone dropped model parameter, so closure needs both the temperature scaling and parsing the model TNOM. RustSpice gives the same 5.3 uA at Vgs=0.8 V as for the .temp 100 deck cap_dev_mos_temp_card [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VDS d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 PHI=0.65 TNOM=60)
.dc VG 0.8 3 0.1
.end
