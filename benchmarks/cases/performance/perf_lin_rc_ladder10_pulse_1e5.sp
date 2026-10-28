RC ladder with 10 stages, pulse drive, 1e5 steps
* bench.id: perf_lin_rc_ladder10_pulse_1e5
* bench.group: performance
* bench.title: RC ladder with 10 stages, pulse drive, 1e5 steps
* bench.probes: v(n1), v(n5), v(n10), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 2n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 12
* bench.est_rss_mib: 35
* bench.tags: linear, tran, jit_tier, long, ladder
* bench.note: tridiagonal 12-unknown system, 1e5 steps; every vector of the ladder is written to the rawfile by both simulators
V1 in 0 PULSE(0 1 10n 1u 1u 50u 100u)
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
.tran 2n 200u
.end
