NMOS whose bulk is driven 1.5 V through 1k: the forward-biased bulk-source junction clamps the bulk at 0.65 V
* bench.id: acc_dc_mos_bulk_junction_forward
* bench.group: accuracy
* bench.title: NMOS whose bulk is driven 1.5 V through 1k: the forward-biased bulk-source junction clamps the bulk at 0.65 V
* bench.probes: v(b), i(vb)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mos, level1, op, bulk_junction, silent_risk
* bench.note: Gap closed 2026-10-28: RustSpice now models level-1 bulk-drain/bulk-source junctions; the evidence below records the reference and the behaviour before the fix. tol.abs 1e-10. Oracle: KCL at b (1.5 V through 1k, IS = 1e-14 A diodes b-s and b-d, gmin across each, CODATA Vt at 300.15 K) solved in mpmath gives v(b) = 0.6508876 V and i(vb) = -0.84911 mA; tight ngspice gives 0.6508873 V (2e-7 V apart, inside tol.rel 1e-6); the MOS stays off (Vth = 0.454 V at Vbs = 0.65 V, Vgs = 0), so only the junctions matter. Slug: the deck contains no junction parameter, the defect is a missing structural element, and silent-ignore-model-param is the closest registered slug (its registry text lists 'no bulk junctions'). Flip: explicit diodes b-0 and b-d (IS = 1e-14) in RustSpice give v(b) = 0.650887 V, 2e-11 V from ngspice; .model is=1e-12 moves ngspice to 0.535 V and leaves RustSpice at 1.5 V (IS is dropped as well)
Vb vb 0 1.5
Rb vb b 1k
Vd d 0 1
Vg g 0 0
M1 d g 0 b nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.op
.end
