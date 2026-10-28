Subcircuit with series resistor and diode instantiated twice, V1 swept 0 to 5 V
* bench.id: cap_xm_sub_dc_sweep_diode_divider
* bench.group: capability
* bench.title: .subckt (R + diode) instantiated twice in a divider, internal nodes x1.k / x2.k probed, V1 swept 0..5 V
* bench.probes: v(x1.k), v(m), v(x2.k), v(o), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.tags: subckt, dc, sweep, diode
* bench.est_rss_mib: 16
* bench.note: Fills the X x .dc cell (1 deck). Both instances own an internal node k, so the flattening must scope it (x1.k = 3.5179 V and x2.k = 1.3884 V at V1 = 5 V); the hand-flattened twin gives bit-identical vectors in both tools (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py). No engine oracle (dc_nonlinear is flat-only): independent 50-digit mpmath oracle of the flat twin agrees with RustSpice to 1.3e-10 V and with tight ngspice to 3.1e-10 V, ngspice as-is is 2.7e-4 V off (checks/element_analysis_matrix_holes/check_independent_oracles.py). At V1 < ~1 V both diodes are off and v(m) = V1/2 is set by the 1e-12 S junction gmin of D1 against D2 + 1k, a convention shared by RustSpice, ngspice and the oracle; that floating node is also the least path-independent value (backward vs forward sweep differs by 69 % of the band in RustSpice, still inside it). Worst probe uses 5.8 % of the band.
.subckt dres p n
R1 p k 2k
D1 k n dm
.ends
.model dm D(IS=1e-14 N=1)
V1 in 0 0
X1 in m dres
X2 m o dres
R2 o 0 1k
.dc V1 0 5 0.25
.end
