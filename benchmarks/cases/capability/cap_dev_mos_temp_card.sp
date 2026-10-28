NMOS L1 at .temp 100 (mobility, threshold and phi shift)
* bench.id: cap_dev_mos_temp_card
* bench.group: capability
* bench.title: NMOS L1 at .temp 100 (mobility, threshold and phi shift)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc, temperature
* bench.note: evidence: RustSpice output is bit-identical with and without .temp 100; tight ngspice is a square law with mobility (T/Tnom)^-1.5 and Vth=0.633 V (implied Vth identical at four Vgs points): 2.14 mA vs RustSpice 2.80 mA at Vgs=3 V [2026-10-28: gap closed; the RustSpice results quoted above predate the fix.]
VDS d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 PHI=0.65)
.temp 100
.dc VG 0.8 3 0.1
.end
