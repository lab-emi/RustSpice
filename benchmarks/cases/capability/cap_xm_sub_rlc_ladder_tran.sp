Three-section RLC ladder built from two levels of nested subcircuits
* bench.id: cap_xm_sub_rlc_ladder_tran
* bench.group: capability
* bench.title: 3-section RLC ladder from nested .subckt rl / sec (L and R inside the inner level), PULSE step response, tstep 0.1 ns
* bench.probes: v(n1), v(n2), v(n3), i(v1), v(x1.x1.m), v(x3.x1.m)
* bench.tol.norm: 1e-3
* bench.expect: match
* bench.tags: subckt, nested, linear, tran, rlc, pulse, npoints
* bench.est_rss_mib: 16
* bench.note: Two-level hierarchy with an inductor inside the inner subcircuit (the corpus subckts contain only R, C, D, M): probes include the nested internal nodes x1.x1.m and x3.x1.m. Oracle is exact, not numerical: the 6-state LTI system is propagated with matrix exponentials over the piecewise-linear PULSE segments (checks/element_analysis_matrix_holes/check_independent_oracles.py); converged ngspice within 5.7e-7 of swing from it, RustSpice 1.3e-4..2.3e-4 at 0.1 ns (9.2e-4 at 0.2 ns, second order), ngspice as-is 1.3e-4..2.3e-4 (same size), so tol.norm 1e-3 (4x) is tighter than the default 5e-3. The engine oracle tran_lti cannot read X cards; the flat twin is identical bit for bit (checks/element_analysis_matrix_holes/check_hierarchy_flat_equivalence.py). Observation, not probed: the inductor branch currents are named i(l.x1.x1.l1) by ngspice and i(l1_x1.x1) by RustSpice, so hierarchical inductor currents cannot be compared by name.
.subckt rl a b
R1 a m 5
L1 m b 100n
.ends
.subckt sec in out
X1 in out rl
C1 out 0 100p
.ends
V1 in 0 PULSE(0 1 5n 2n 2n 40n 100n)
X1 in n1 sec
X2 n1 n2 sec
X3 n2 n3 sec
RL n3 0 50
.tran 0.1n 100n
.end
