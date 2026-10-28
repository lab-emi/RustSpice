Diode-clamped RC ladder with 5 stages, 20 kHz sine, 1e6 steps
* bench.id: perf_scal_tran_len_diode_clamp_ladder5_1e6
* bench.group: performance
* bench.title: Diode-clamped RC ladder with 5 stages, 20 kHz sine, 1e6 steps
* bench.probes: v(n1), v(n3), v(n5), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 1000000
* bench.est_rss_mib: 130
* bench.tags: nonlinear, tran, diode, jit_tier, long, ladder, npoints, length_axis, family:perf_scal_tran_len_diode_clamp_ladder5
* bench.note: length member 1e6 of the nonlinear family (same lengths as the linear family); the Radau oracle covers the first 1 ms, later periods repeat; rustspice keeps all 9 vectors x 1e6 rows in memory (VmHWM 99 MiB) while ngspice streams (15 MiB); measured on an idle core: rustspice e2e 1.13 s (solve 1.08 s) vs ngspice SPARSE 4.13 s and KLU 4.34 s; end-to-end exponent over the family 0.94 (rustspice) and 0.96 (ngspice)
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
.tran 100n 100.00005m
.end
