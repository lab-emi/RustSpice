Diode-clamped RC ladder with 5 stages, 20 kHz sine, 1e4 steps
* bench.id: perf_scal_tran_len_diode_clamp_ladder5_1e4
* bench.group: performance
* bench.title: Diode-clamped RC ladder with 5 stages, 20 kHz sine, 1e4 steps
* bench.probes: v(n1), v(n3), v(n5), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 10000
* bench.est_rss_mib: 26
* bench.tags: nonlinear, tran, diode, jit_tier, long, ladder, npoints, length_axis, family:perf_scal_tran_len_diode_clamp_ladder5
* bench.note: nonlinear counterpart of the RC-ladder length family: every node of a 5-stage ladder (R 1k, C 1n) carries a diode to ground and the ladder is driven by a 3 V 20 kHz sine, so every step is a Newton solve; the circuit is a pure ODE system, so scipy Radau (rtol 1e-11, analytic Jacobian, independent of both tools) over the first 1 ms is an oracle: rustspice 6.1e-5 and ngspice 1.5e-5 of the swing (the harness number against the tight ngspice reference is larger, 4.5e-4, because it includes the 2.4e-4 resampling floor of ngspice's irregular rows); scaling family perf_scal_tran_len_diode_clamp_ladder5 (size = steps); measured on an idle core (median of 5): rustspice e2e 14.9 ms (solve 11.6 ms) vs ngspice SPARSE 49.4 ms and KLU 52.1 ms
.model dm D(IS=1e-14 N=1)
V1 in 0 SIN(0 3 20k)
R1 in n1 1k
C1 n1 0 1n
D1 n1 0 dm
R2 n1 n2 1k
C2 n2 0 1n
D2 n2 0 dm
R3 n2 n3 1k
C3 n3 0 1n
D3 n3 0 dm
R4 n3 n4 1k
C4 n4 0 1n
D4 n4 0 dm
R5 n4 n5 1k
C5 n5 0 1n
D5 n5 0 dm
.tran 100n 1.00005m
.end
