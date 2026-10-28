RC ladder, 10 stages (R 1k, C 1p), cards shuffled (seed 1)
* bench.id: rob_ord_rcladder10_cardshuffle_s1
* bench.group: robustness
* bench.title: RC ladder, 10 stages (R 1k, C 1p), cards shuffled (seed 1)
* bench.probes: v(a1), v(a2), v(a3), v(a5), v(a8), v(a10)
* bench.tol.norm: 2e-4
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 12
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, control, regression_anchor, jit_tier
* bench.note: card order shuffled with random.Random(1); small ladders tolerate card shuffling (8 of 8 seeds within 2.3e-5 V of the exact solution at 5 stages and within 1.3e-5 V at 10 stages, natural order 1.3e-5 V), so this is the size-axis control of the failing 20/40/60-stage decks; card order is the only difference to the natural-order deck (node names unchanged); reference = converged ngspice (reltol 1e-6, tmax tstep/20), oracle = exact matrix-exponential solution of the piecewise-linear drive
C10 a10 0 1p
R9 a8 a9 1k
R10 a9 a10 1k
R6 a5 a6 1k
R3 a2 a3 1k
R5 a4 a5 1k
V1 in 0 PULSE(0 1 1n 100p 100p 12n 50n)
C7 a7 0 1p
R1 in a1 1k
C8 a8 0 1p
C3 a3 0 1p
C5 a5 0 1p
R7 a6 a7 1k
R4 a3 a4 1k
C6 a6 0 1p
R8 a7 a8 1k
R2 a1 a2 1k
C4 a4 0 1p
C1 a1 0 1p
C9 a9 0 1p
C2 a2 0 1p
.tran 10p 20n
.end
