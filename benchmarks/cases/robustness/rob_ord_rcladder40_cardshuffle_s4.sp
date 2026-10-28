RC ladder, 40 stages (R 1k, C 1p), cards shuffled (seed 4)
* bench.id: rob_ord_rcladder40_cardshuffle_s4
* bench.group: robustness
* bench.title: RC ladder, 40 stages (R 1k, C 1p), cards shuffled (seed 4)
* bench.probes: v(a1), v(a2), v(a3), v(a5), v(a8), v(a10), v(a15), v(a20)
* bench.tol.norm: 2e-4
* bench.floor: 5e-4
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 42
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, silent_risk, wrong_values
* bench.note: card order shuffled with random.Random(4); card order is the only difference to the natural-order deck (node names unchanged); reference = converged ngspice (reltol 1e-6, tmax tstep/20), oracle = exact matrix-exponential solution of the piecewise-linear drive
R39 a38 a39 1k
C32 a32 0 1p
R35 a34 a35 1k
C36 a36 0 1p
R2 a1 a2 1k
C13 a13 0 1p
R37 a36 a37 1k
C18 a18 0 1p
R13 a12 a13 1k
R22 a21 a22 1k
C29 a29 0 1p
R24 a23 a24 1k
R29 a28 a29 1k
C21 a21 0 1p
C2 a2 0 1p
C10 a10 0 1p
C34 a34 0 1p
C17 a17 0 1p
C7 a7 0 1p
R15 a14 a15 1k
R14 a13 a14 1k
R17 a16 a17 1k
R23 a22 a23 1k
R30 a29 a30 1k
C38 a38 0 1p
C27 a27 0 1p
C28 a28 0 1p
R5 a4 a5 1k
R34 a33 a34 1k
R32 a31 a32 1k
R16 a15 a16 1k
C24 a24 0 1p
V1 in 0 PULSE(0 1 1n 100p 100p 12n 50n)
R25 a24 a25 1k
C22 a22 0 1p
C31 a31 0 1p
C40 a40 0 1p
R20 a19 a20 1k
C37 a37 0 1p
R8 a7 a8 1k
C16 a16 0 1p
C12 a12 0 1p
R11 a10 a11 1k
R40 a39 a40 1k
R3 a2 a3 1k
R12 a11 a12 1k
R33 a32 a33 1k
C20 a20 0 1p
C9 a9 0 1p
R38 a37 a38 1k
C5 a5 0 1p
C6 a6 0 1p
R9 a8 a9 1k
R28 a27 a28 1k
C30 a30 0 1p
R36 a35 a36 1k
R21 a20 a21 1k
R27 a26 a27 1k
R1 in a1 1k
C39 a39 0 1p
C8 a8 0 1p
C3 a3 0 1p
C26 a26 0 1p
C11 a11 0 1p
R18 a17 a18 1k
C23 a23 0 1p
C33 a33 0 1p
C14 a14 0 1p
R4 a3 a4 1k
R19 a18 a19 1k
C35 a35 0 1p
R26 a25 a26 1k
C1 a1 0 1p
C4 a4 0 1p
R6 a5 a6 1k
R10 a9 a10 1k
R31 a30 a31 1k
C25 a25 0 1p
R7 a6 a7 1k
C19 a19 0 1p
C15 a15 0 1p
.tran 10p 20n
.end
