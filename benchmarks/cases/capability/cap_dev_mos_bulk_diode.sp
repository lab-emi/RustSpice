NMOS bulk-source/drain junctions conduct at Vbs=0.5..0.7 V
* bench.id: cap_dev_mos_bulk_diode
* bench.group: capability
* bench.title: NMOS bulk-source/drain junctions conduct at Vbs=0.5..0.7 V
* bench.probes: i(vb)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: no bulk junction diodes: i(vb)=0 at Vbs=0.7 V (ngspice -11.3 mA)
* bench.tags: mosfet, level1, dc
* bench.note: evidence: RustSpice i(vb)=0 at every point; tight ngspice i(vb) at Vbs=0.7 V equals two ideal IS=1e-14 junction diodes (11.34 mA, the same value as twice a 1e-14 diode in cap_dev_dio_area); the deck sets no junction parameter (nothing is parsed and dropped): it relies on ngspice's default junction saturation current, which is why the slug here means the missing bulk-junction sub-model (adding IS=1e-12 to the MOS model scales ngspice's i(vb) by 100x, IS=0 JS=0 removes it to 1.4e-12 A, RustSpice stays at exactly 0 A in all three)
VDS d 0 0
VG g 0 0
VB b 0 0
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)
.dc VB 0 0.7 0.05
.end
