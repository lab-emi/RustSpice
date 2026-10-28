Cards of perf_lin_rc_ladder30_pulse_1e5 shuffled, tstop cut to 20 us (1e4 steps)
* bench.id: rob_ord_perm_perf_lin_rc_ladder30_pulse
* bench.group: robustness
* bench.title: Cards of perf_lin_rc_ladder30_pulse_1e5 shuffled, tstop cut to 20 us (1e4 steps)
* bench.probes: v(n1), v(n15), v(n30), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.oracle: tran_lti
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 2n
* bench.size: 32
* bench.est_rss_mib: 20
* bench.tags: linear, tran, ladder, rc, card_order, silent_risk, corpus_copy
* bench.note: metamorphic copy of the passing corpus deck perf_lin_rc_ladder30_pulse_1e5 (30 stages, R 1k, C 100p, PULSE 0..1 V): identical circuit, cards shuffled by random.Random(3), .tran stops at 20 us instead of 200 us; tolerance and oracle as in the original
C10 n10 0 100p
R9 n8 n9 1k
R3 n2 n3 1k
C11 n11 0 100p
C13 n13 0 100p
C28 n28 0 100p
R16 n15 n16 1k
R14 n13 n14 1k
C3 n3 0 100p
R24 n23 n24 1k
R2 n1 n2 1k
R4 n3 n4 1k
C1 n1 0 100p
R20 n19 n20 1k
C14 n14 0 100p
R6 n5 n6 1k
C16 n16 0 100p
C26 n26 0 100p
R15 n14 n15 1k
R11 n10 n11 1k
R7 n6 n7 1k
C22 n22 0 100p
R10 n9 n10 1k
C30 n30 0 100p
R29 n28 n29 1k
R21 n20 n21 1k
R27 n26 n27 1k
C18 n18 0 100p
R1 in n1 1k
C9 n9 0 100p
C21 n21 0 100p
C5 n5 0 100p
R26 n25 n26 1k
R25 n24 n25 1k
C12 n12 0 100p
R17 n16 n17 1k
C25 n25 0 100p
R23 n22 n23 1k
R5 n4 n5 1k
R13 n12 n13 1k
R22 n21 n22 1k
C23 n23 0 100p
C29 n29 0 100p
C24 n24 0 100p
C6 n6 0 100p
C7 n7 0 100p
R18 n17 n18 1k
C8 n8 0 100p
C27 n27 0 100p
V1 in 0 PULSE(0 1 10n 1u 1u 50u 100u)
R28 n27 n28 1k
C2 n2 0 100p
R30 n29 n30 1k
C20 n20 0 100p
C15 n15 0 100p
C19 n19 0 100p
R12 n11 n12 1k
C4 n4 0 100p
C17 n17 0 100p
R19 n18 n19 1k
R8 n7 n8 1k
.tran 2n 20u
.end
