PMOS held off with its drain on a 1 kohm node to ground: the reverse bulk-drain junction saturation current IS = 1e-6 A sets v(x) = 1.0 mV
* bench.id: acc_dc_mos_bulk_leakage_is_dominant
* bench.group: accuracy
* bench.title: PMOS held off with its drain on a 1 kohm node to ground: the reverse bulk-drain junction saturation current IS = 1e-6 A sets v(x) = 1.0 mV
* bench.probes: v(x)
* bench.floor: 1e-6
* bench.expect: match
* bench.tags: mos, level1, op, bulk_junction, leakage, silent_risk
* bench.note: Gap closed 2026-10-28: RustSpice now models level-1 bulk-drain/bulk-source junctions; the evidence below records the reference and the behaviour before the fix. closed form (mpmath KCL at x, CODATA Vt at 300.15 K): IS*(1 - exp((v(x) - Vdd)/Vt)) + gmin*(Vdd - v(x)) = v(x)/R gives v(x) = R*(IS + gmin*Vdd) = 1.000000999e-3 V with gmin = 1e-12 S across the junction (tight ngspice 1.000000999e-3 V) and exactly 1.0e-3 V without gmin (ngspice with .options gmin=0), while a drain-source gmin only (RustSpice) gives 1.0e-9 V. The gmin convention moves the reference by 1e-9 V, inside the tolerance 2e-9 V (tol.abs 1e-9 + tol.rel 1e-6), so unlike acc_dc_mos_bulk_leakage_hiz_node (1 Gohm, IS = 1e-14 A, where gmin is 99 % of the reading) a RustSpice that adds the junction closes the gap whichever way its gmin is placed
Vdd vdd 0 1
Rpd x 0 1k
Vg g vdd 0
M1 x g vdd vdd pm w=10u l=1u
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04 is=1e-6)
.op
.end
