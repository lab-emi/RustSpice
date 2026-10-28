Current-source sweep through an inductor into a resistor-diode load
* bench.id: cap_xm_l_dc_sweep_isrc_diode
* bench.group: capability
* bench.title: I1 swept 0..2 mA into a node that is tied to the circuit only through an inductor (DC short), 1k || diode load
* bench.probes: v(a), v(b), i(l1)
* bench.expect: match
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear
* bench.tags: passive, inductor, dc, sweep, diode, isrc
* bench.est_rss_mib: 16
* bench.note: Node a touches only I1 and L1, so the DC solution exists only because the inductor is stamped as a short branch (an open-circuit L would leave a current source into a floating node): v(a)=v(b) and i(l1)=I1 at all 17 points while the 1k || diode load turns from resistive (0.125 V per 125 uA) to the diode knee (0.6626 V at 2 mA). Independent mpmath oracle: RustSpice 1.0e-10 V, tight ngspice 4.7e-10 V from it; engine oracle covers v(a), v(b) only (i(l1) unsupported there, covered by checks/element_analysis_matrix_holes/check_independent_oracles.py; i(l1) is exactly the sweep value in all three tools). Worst probe uses 7.2 % of the tolerance band.
I1 0 a 0
L1 a b 1m
R1 b 0 1k
D1 b 0 dm
.model dm D(IS=1e-14 N=1)
.dc I1 0 2m 0.125m
.end
