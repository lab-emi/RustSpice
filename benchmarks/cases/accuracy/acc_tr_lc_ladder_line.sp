LC ladder (lumped transmission line) with reflections
* bench.id: acc_tr_lc_ladder_line
* bench.group: accuracy
* bench.title: 10-section LC ladder (Z0 = 31.6 ohm, td ~ 3.2 ns) matched at the source, 100 ohm load, 1 ns edges, 25 ps steps
* bench.probes: v(a5), v(a10)
* bench.tol.norm: 3e-3
* bench.oracle: tran_lti
* bench.tags: linear, tran, lc, coupled
* bench.est_rss_mib: 16
Vin in 0 PULSE(0 1 0 1n 1n 8n 20n)
Rs in a0 31.6
L1 a0 a1 10n
C1 a1 0 10p
L2 a1 a2 10n
C2 a2 0 10p
L3 a2 a3 10n
C3 a3 0 10p
L4 a3 a4 10n
C4 a4 0 10p
L5 a4 a5 10n
C5 a5 0 10p
L6 a5 a6 10n
C6 a6 0 10p
L7 a6 a7 10n
C7 a7 0 10p
L8 a7 a8 10n
C8 a8 0 10p
L9 a8 a9 10n
C9 a9 0 10p
L10 a9 a10 10n
C10 a10 0 10p
Rload a10 0 100
.tran 25p 30n
.end
