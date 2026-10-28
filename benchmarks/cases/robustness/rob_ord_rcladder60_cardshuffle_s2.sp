RC ladder, 60 stages (R 1k, C 1p), cards shuffled (seed 2)
* bench.id: rob_ord_rcladder60_cardshuffle_s2
* bench.group: robustness
* bench.title: RC ladder, 60 stages (R 1k, C 1p), cards shuffled (seed 2)
* bench.probes: v(a1), v(a2), v(a3), v(a5), v(a8), v(a10), v(a15), v(a20)
* bench.tol.norm: 2e-4
* bench.floor: 5e-4
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 62
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, silent_risk, non_finite
* bench.note: card order shuffled with random.Random(2); probes stop at a20 (the far nodes stay below 5e-4 V in 20 ns and would be vacuous); card order is the only difference to the natural-order deck (node names unchanged); reference = converged ngspice (reltol 1e-6, tmax tstep/20), oracle = exact matrix-exponential solution of the piecewise-linear drive
C59 a59 0 1p
C49 a49 0 1p
C4 a4 0 1p
C1 a1 0 1p
C19 a19 0 1p
R45 a44 a45 1k
C6 a6 0 1p
C31 a31 0 1p
C38 a38 0 1p
V1 in 0 PULSE(0 1 1n 100p 100p 12n 50n)
R3 a2 a3 1k
R1 in a1 1k
C30 a30 0 1p
C45 a45 0 1p
C60 a60 0 1p
R59 a58 a59 1k
R26 a25 a26 1k
C3 a3 0 1p
R46 a45 a46 1k
R7 a6 a7 1k
C9 a9 0 1p
R19 a18 a19 1k
R60 a59 a60 1k
C12 a12 0 1p
R53 a52 a53 1k
C26 a26 0 1p
C8 a8 0 1p
C13 a13 0 1p
C34 a34 0 1p
C33 a33 0 1p
R5 a4 a5 1k
R10 a9 a10 1k
C39 a39 0 1p
R25 a24 a25 1k
C43 a43 0 1p
C50 a50 0 1p
C7 a7 0 1p
C29 a29 0 1p
C22 a22 0 1p
R18 a17 a18 1k
C18 a18 0 1p
C48 a48 0 1p
R22 a21 a22 1k
R31 a30 a31 1k
C21 a21 0 1p
R37 a36 a37 1k
R50 a49 a50 1k
R54 a53 a54 1k
R49 a48 a49 1k
C36 a36 0 1p
R16 a15 a16 1k
R8 a7 a8 1k
R17 a16 a17 1k
C51 a51 0 1p
C52 a52 0 1p
C56 a56 0 1p
R32 a31 a32 1k
R13 a12 a13 1k
C42 a42 0 1p
C58 a58 0 1p
C14 a14 0 1p
R42 a41 a42 1k
C41 a41 0 1p
R23 a22 a23 1k
R47 a46 a47 1k
R27 a26 a27 1k
R29 a28 a29 1k
R12 a11 a12 1k
C35 a35 0 1p
R51 a50 a51 1k
C40 a40 0 1p
R48 a47 a48 1k
R9 a8 a9 1k
R38 a37 a38 1k
R21 a20 a21 1k
R40 a39 a40 1k
C44 a44 0 1p
R15 a14 a15 1k
C15 a15 0 1p
C11 a11 0 1p
R36 a35 a36 1k
R57 a56 a57 1k
R34 a33 a34 1k
C27 a27 0 1p
C24 a24 0 1p
C20 a20 0 1p
R30 a29 a30 1k
R58 a57 a58 1k
R2 a1 a2 1k
R56 a55 a56 1k
C17 a17 0 1p
C32 a32 0 1p
C28 a28 0 1p
R35 a34 a35 1k
R24 a23 a24 1k
R33 a32 a33 1k
C46 a46 0 1p
C25 a25 0 1p
R41 a40 a41 1k
R28 a27 a28 1k
C10 a10 0 1p
R44 a43 a44 1k
C37 a37 0 1p
C2 a2 0 1p
C57 a57 0 1p
R14 a13 a14 1k
R39 a38 a39 1k
C16 a16 0 1p
R20 a19 a20 1k
R55 a54 a55 1k
R43 a42 a43 1k
R52 a51 a52 1k
C47 a47 0 1p
R11 a10 a11 1k
C53 a53 0 1p
C23 a23 0 1p
C5 a5 0 1p
R6 a5 a6 1k
R4 a3 a4 1k
C54 a54 0 1p
C55 a55 0 1p
.tran 10p 20n
.end
