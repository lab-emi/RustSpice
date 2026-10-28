Waveform sources declared inside a subcircuit drive an RC network
* bench.id: cap_xm_sub_source_pulse_sin_tran
* bench.group: capability
* bench.title: PULSE voltage source and SIN current source declared inside a .subckt, coupled RC network, tstep 1 ns
* bench.probes: v(a), v(b), v(out)
* bench.tol.norm: 3e-3
* bench.expect: match
* bench.tags: subckt, source, waveform, linear, tran, pulse, sine, npoints
* bench.est_rss_mib: 16
* bench.note: Positive anchor for waveform sources that live inside a .subckt (V PULSE and I SIN, instance named X1; the failing instance-name variant is the known-gap deck cap_x_sub_instance_name_waveform_tran). v(a) is the PULSE itself (exact to 3e-15 of swing), v(b) and v(out) see the SIN current through R2/C2 and the pulse through R1. Independent oracle (hand-derived 2-state ODE, scipy Radau rtol 1e-12, restarted at the pulse corners, checks/element_analysis_matrix_holes/check_independent_oracles.py): converged ngspice within 9.5e-7 of swing, RustSpice 1.29e-3 on v(b) (worst at the first 1 ns step: BDF1 start; ngspice as-is 1.5e-4) and 4.0e-4 on v(out) (ngspice as-is 2.7e-4); tol.norm 3e-3. Hand-flattened twin identical bit for bit (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py).
.subckt drv a b
V1 a 0 PULSE(0 2 10n 5n 5n 40n 100n)
I1 0 b SIN(0 1m 10meg 0)
.ends
X1 a b drv
R1 a out 1k
R2 b out 2k
R3 out 0 1k
C1 out 0 20p
C2 b 0 5p
.tran 1n 200n
.end
