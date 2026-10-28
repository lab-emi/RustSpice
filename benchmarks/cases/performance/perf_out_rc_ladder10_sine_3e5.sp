RC ladder with 10 stages, 5 kHz sine, 3e5 steps (13 vectors per row)
* bench.id: perf_out_rc_ladder10_sine_3e5
* bench.group: performance
* bench.title: RC ladder with 10 stages, 5 kHz sine, 3e5 steps (13 vectors per row)
* bench.probes: v(n1), v(n10), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 66.666667n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 12
* bench.est_rss_mib: 75
* bench.tags: linear, tran, jit_tier, long, wide_output, rawfile_write
* bench.note: 3e5 output rows of 13 vectors (about 31 MB of rawfile): row-writing throughput is a large share of the wall time
V1 in 0 SIN(0 1 5k)
R1 in n1 1k
C1 n1 0 100n
R2 n1 n2 1k
C2 n2 0 100n
R3 n2 n3 1k
C3 n3 0 100n
R4 n3 n4 1k
C4 n4 0 100n
R5 n4 n5 1k
C5 n5 0 100n
R6 n5 n6 1k
C6 n6 0 100n
R7 n6 n7 1k
C7 n7 0 100n
R8 n7 n8 1k
C8 n8 0 100n
R9 n8 n9 1k
C9 n9 0 100n
R10 n9 n10 1k
C10 n10 0 100n
.tran 66.666667n 20m
.end
