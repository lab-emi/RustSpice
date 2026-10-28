PMOS held off with its drain on a 1 Gohm node to ground: the reverse bulk-drain junction leakage (IS = 1e-14 A) moves v(x) by 1 %
* bench.id: acc_dc_mos_bulk_leakage_hiz_node
* bench.group: accuracy
* bench.title: PMOS held off with its drain on a 1 Gohm node to ground: the reverse bulk-drain junction leakage (IS = 1e-14 A) moves v(x) by 1 %
* bench.probes: v(x)
* bench.floor: 1e-6
* bench.expect: either
* bench.tags: mos, level1, op, bulk_junction, gmin, leakage, silent_risk
* bench.note: neither tool can be judged: 99 % of v(x) = 1.0 mV is the gmin convention, not the junction. mpmath KCL with the junction saturation current alone (physical truth) gives v(x) = 1.0e-5 V; ngspice reproduces it with .options gmin=0 (1.0000e-5 V) and gives 1.00899e-3 V by default because its 1e-12 S gmin sits across the bulk-drain junction (G = gmin + IS = 1.01e-12 S, v(x) = Vdd*G/(G + 1/R)); RustSpice has no junction, puts the 1e-12 S gmin drain-source inside the MOS and ignores .options gmin (0.99900e-3 V, G = 1e-12 S). Both defaults are 100x off the physical truth and differ from each other only by the 1 % IS term; adding the missing junction to RustSpice (explicit diode with its own kernel) gives 2.006e-3 V because the gmin is then counted twice, so this observable cannot detect closure either. The missing leak is tested cleanly by acc_dc_mos_bulk_leakage_is_dominant (IS = 1e-6 A on 1 kohm)
Vdd vdd 0 1
Rpd x 0 1g
Vg g vdd 0
M1 x g vdd vdd pm w=10u l=1u
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.op
.end
