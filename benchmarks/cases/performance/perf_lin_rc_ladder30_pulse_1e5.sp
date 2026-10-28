RC ladder with 30 stages, pulse drive, 1e5 steps
* bench.id: perf_lin_rc_ladder30_pulse_1e5
* bench.group: performance
* bench.title: RC ladder with 30 stages, pulse drive, 1e5 steps
* bench.probes: v(n1), v(n15), v(n30), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 2n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 32
* bench.est_rss_mib: 55
* bench.tags: linear, tran, jit_tier, long, ladder, wide_output
* bench.note: 32 unknowns, the largest size the Cranelift JIT tier takes; 33 vectors are written per row
V1 in 0 PULSE(0 1 10n 1u 1u 50u 100u)
R1 in n1 1k
C1 n1 0 100p
R2 n1 n2 1k
C2 n2 0 100p
R3 n2 n3 1k
C3 n3 0 100p
R4 n3 n4 1k
C4 n4 0 100p
R5 n4 n5 1k
C5 n5 0 100p
R6 n5 n6 1k
C6 n6 0 100p
R7 n6 n7 1k
C7 n7 0 100p
R8 n7 n8 1k
C8 n8 0 100p
R9 n8 n9 1k
C9 n9 0 100p
R10 n9 n10 1k
C10 n10 0 100p
R11 n10 n11 1k
C11 n11 0 100p
R12 n11 n12 1k
C12 n12 0 100p
R13 n12 n13 1k
C13 n13 0 100p
R14 n13 n14 1k
C14 n14 0 100p
R15 n14 n15 1k
C15 n15 0 100p
R16 n15 n16 1k
C16 n16 0 100p
R17 n16 n17 1k
C17 n17 0 100p
R18 n17 n18 1k
C18 n18 0 100p
R19 n18 n19 1k
C19 n19 0 100p
R20 n19 n20 1k
C20 n20 0 100p
R21 n20 n21 1k
C21 n21 0 100p
R22 n21 n22 1k
C22 n22 0 100p
R23 n22 n23 1k
C23 n23 0 100p
R24 n23 n24 1k
C24 n24 0 100p
R25 n24 n25 1k
C25 n25 0 100p
R26 n25 n26 1k
C26 n26 0 100p
R27 n26 n27 1k
C27 n27 0 100p
R28 n27 n28 1k
C28 n28 0 100p
R29 n28 n29 1k
C29 n29 0 100p
R30 n29 n30 1k
C30 n30 0 100p
.tran 2n 200u
.end
