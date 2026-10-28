Inductor as a DC short in an R-L-R-diode path, V1 swept 0 to 5 V
* bench.id: cap_xm_l_dc_sweep_diode
* bench.group: capability
* bench.title: Inductor (DC short) in a series R-L-R-diode path, V1 swept 0..5 V
* bench.probes: v(b), v(c), v(d), i(v1), i(l1)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear
* bench.tags: passive, inductor, dc, sweep, diode
* bench.est_rss_mib: 16
* bench.note: Fills the empty L x .dc cell (the corpus had 10 L .op and 20 L .tran decks, none with .dc). L1 must be a short at every sweep point: v(b)=v(c) and i(l1)=-i(v1) while the diode turns on between 0.5 V (i(v1) = -2.19 uA) and 5 V (-2.878 mA, v(d) = 0.6825 V). Independent 50-digit mpmath MNA oracle (checks/element_analysis_matrix_holes/check_independent_oracles.py) agrees with RustSpice to 4.7e-11 V and with tight ngspice to 2.6e-10 V (worst probe v(d); ngspice as-is is 2.2e-4 V off); the engine oracle dc_nonlinear covers every probe except i(l1) (it cannot evaluate inductor currents), so the case-level oracle status reads unsupported by design and i(l1) is covered by the python oracle. Band tol.rel 1e-8 + tol.abs 1e-12: worst probe uses 3.8 % of it. Path independence (sweep point == stand-alone .op, backward == forward): checks/element_analysis_matrix_holes/check_dc_sweep_vs_op.py.
V1 a 0 0
R1 a b 1k
L1 b c 10m
R2 c d 500
D1 d 0 dm
.model dm D(IS=1e-14 N=1)
.dc V1 0 5 0.5
.end
