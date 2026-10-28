LC ladder, 10 sections (L 1n, C 1p, 50 ohm source and load), cards shuffled (seed 1)
* bench.id: rob_ord_lcladder10_cardshuffle_s1
* bench.group: robustness
* bench.title: LC ladder, 10 sections (L 1n, C 1p, 50 ohm source and load), cards shuffled (seed 1)
* bench.probes: v(a1), v(a3), v(a5), v(a8), v(a10)
* bench.tol.norm: 5e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.size: 22
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, lc, card_order, silent_risk, non_finite
* bench.note: card order shuffled with random.Random(1); natural order is within 8.4e-4 V of the exact solution (fixed-step trapezoid), hence tol.norm 5e-3; reference = converged ngspice, oracle = exact matrix-exponential solution
L10 a9 a10 1n
C2 a2 0 1p
L8 a7 a8 1n
C4 a4 0 1p
C9 a9 0 1p
Rl a10 0 50
C5 a5 0 1p
C6 a6 0 1p
V1 s 0 PULSE(0 1 0 100p 100p 1n 10n)
C3 a3 0 1p
Rs s a0 50
C10 a10 0 1p
L6 a5 a6 1n
L3 a2 a3 1n
L5 a4 a5 1n
C8 a8 0 1p
L7 a6 a7 1n
C7 a7 0 1p
C1 a1 0 1p
L4 a3 a4 1n
L1 a0 a1 1n
L9 a8 a9 1n
L2 a1 a2 1n
.tran 2p 2n
.end
