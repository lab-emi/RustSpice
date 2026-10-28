RC ladder with 10 stages, 20 kHz sine, 1e6 steps
* bench.id: perf_scal_tran_len_rc_ladder10_1e6
* bench.group: performance
* bench.title: RC ladder with 10 stages, 20 kHz sine, 1e6 steps
* bench.probes: v(n1), v(n5), v(n10), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 1000000
* bench.est_rss_mib: 190
* bench.tags: linear, tran, jit_tier, long, ladder, npoints, length_axis, family:perf_scal_tran_len_rc_ladder10
* bench.note: length member 1e6 of the family (the engine's tran_lti oracle exceeds its work budget at this length, so no bench.oracle is set; the closed form on a 2000-sample subset gives rustspice 8e-6 and ngspice 3e-6 of the swing); rustspice keeps every output row in memory (13 vectors x 1e6 rows x 8 B = 104 MB, VmHWM 145 MiB) while ngspice streams to the rawfile (VmHWM 15 MiB); measured on an idle core: rustspice e2e 540 ms (solve 468 ms) vs ngspice SPARSE 3.50 s and KLU 3.82 s; end-to-end exponent over the family 0.93 (rustspice) and 0.96 (ngspice)
V1 in 0 SIN(0 1 20k)
R1 in n1 1k
C1 n1 0 1n
R2 n1 n2 1k
C2 n2 0 1n
R3 n2 n3 1k
C3 n3 0 1n
R4 n3 n4 1k
C4 n4 0 1n
R5 n4 n5 1k
C5 n5 0 1n
R6 n5 n6 1k
C6 n6 0 1n
R7 n6 n7 1k
C7 n7 0 1n
R8 n7 n8 1k
C8 n8 0 1n
R9 n8 n9 1k
C9 n9 0 1n
R10 n9 n10 1k
C10 n10 0 1n
.tran 100n 100.00005m
.end
