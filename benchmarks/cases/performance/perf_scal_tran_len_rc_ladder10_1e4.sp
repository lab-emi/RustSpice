RC ladder with 10 stages, 20 kHz sine, 1e4 steps
* bench.id: perf_scal_tran_len_rc_ladder10_1e4
* bench.group: performance
* bench.title: RC ladder with 10 stages, 20 kHz sine, 1e4 steps
* bench.probes: v(n1), v(n5), v(n10), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 100n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 10000
* bench.est_rss_mib: 26
* bench.tags: linear, tran, jit_tier, long, ladder, npoints, length_axis, family:perf_scal_tran_len_rc_ladder10
* bench.note: the same 10-stage RC ladder (R 1k, C 1n, 20 kHz sine, tstep 100 ns) at 1e4, 1e5 and 1e6 steps so the exponent of the per-step cost can be fitted (scaling family perf_scal_tran_len_rc_ladder10, size = steps); tstop is half a step past the last grid point so no sliver step is produced; the engine tran_lti oracle gives rustspice 7.3e-4 and ngspice 1.1e-4 of the swing (worst probe i(v1)), a closed-form modal solution (eigendecomposition of the 10x10 conductance matrix, no step size) agrees; measured on an idle core (median of 5): rustspice e2e 7.6 ms (solve 3.3 ms) vs ngspice SPARSE 41 ms
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
.tran 100n 1.00005m
.end
