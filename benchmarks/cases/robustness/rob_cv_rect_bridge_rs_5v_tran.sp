Floating 5 V / 1 kHz source with 10 ohm series resistance feeding a diode bridge (100 uF / 1k load, 100k common-mode resistors)
* bench.id: rob_cv_rect_bridge_rs_5v_tran
* bench.group: robustness
* bench.title: Floating 5 V / 1 kHz source with 10 ohm series resistance feeding a diode bridge (100 uF / 1k load, 100k common-mode resistors)
* bench.probes: v(p)
* bench.tol.norm: 5e-4
* bench.expect: match
* bench.tags: nonlinear, rectifier, floating_source, tran
* bench.note: Companion to rob_cv_rect_bridge_floating_tran that adds a realistic 10 ohm source resistance and 100k common-mode resistors to ground (in the original the source terminals reach ground only through the bridge diodes). Reference verified: independent scipy Radau integration (state = C1 voltage, the two bridge input nodes solved by a damped 2-D Newton per RHS call, f64, rtol 1e-10, gmin 1e-12 included); RustSpice (1001 points) and ngspice as-is both sit 4.8e-5 of the 2.76 V swing from it, tight ngspice (reltol 1e-6, 0.25u max step) 6.5e-8, so tol.norm 5e-4 leaves 10x margin and would catch a wrong diode orientation or a lost conduction pulse. The 2.76 V peak stays inside the RustSpice transient clamp (+-5 V around the source), so this is the clean control for the 10 V variant.
Vs a1 b SIN(0 5 1k)
Rs a1 a 10
Rg1 a 0 100k
Rg2 b 0 100k
D1 a p dm
D2 b p dm
D3 0 a dm
D4 0 b dm
C1 p 0 100u
R1 p 0 1k
.model dm D(is=1e-14 n=1)
.tran 5u 5m
.end
