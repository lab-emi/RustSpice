RC ladder, 20 stages (R 1k, C 1p), cards shuffled (seed 1)
* bench.id: rob_ord_rcladder20_cardshuffle_s1
* bench.group: robustness
* bench.title: RC ladder, 20 stages (R 1k, C 1p), cards shuffled (seed 1)
* bench.probes: v(a1), v(a2), v(a3), v(a5), v(a8), v(a10), v(a15), v(a20)
* bench.tol.norm: 2e-4
* bench.floor: 5e-4
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 22
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, silent_risk, jit_tier
* bench.note: card order shuffled with random.Random(1); 22 unknowns, below every size the slug text of unrolled-lu-tran-divergence mentions; card order is the only difference to the natural-order deck (node names unchanged); reference = converged ngspice (reltol 1e-6, tmax tstep/20), oracle = exact matrix-exponential solution of the piecewise-linear drive
C16 a16 0 1p
C1 a1 0 1p
R6 a5 a6 1k
R1 in a1 1k
C19 a19 0 1p
R14 a13 a14 1k
R3 a2 a3 1k
R5 a4 a5 1k
R9 a8 a9 1k
C3 a3 0 1p
R13 a12 a13 1k
C10 a10 0 1p
R19 a18 a19 1k
R11 a10 a11 1k
R15 a14 a15 1k
R12 a11 a12 1k
C5 a5 0 1p
R17 a16 a17 1k
C9 a9 0 1p
R20 a19 a20 1k
C20 a20 0 1p
C7 a7 0 1p
C11 a11 0 1p
C17 a17 0 1p
R10 a9 a10 1k
R18 a17 a18 1k
C6 a6 0 1p
C13 a13 0 1p
V1 in 0 PULSE(0 1 1n 100p 100p 12n 50n)
R8 a7 a8 1k
R2 a1 a2 1k
R7 a6 a7 1k
C12 a12 0 1p
C15 a15 0 1p
C14 a14 0 1p
R16 a15 a16 1k
R4 a3 a4 1k
C8 a8 0 1p
C2 a2 0 1p
C18 a18 0 1p
C4 a4 0 1p
.tran 10p 20n
.end
