RC ladder with 60 stages, pulse drive, 1e5 steps
* bench.id: perf_lin_rc_ladder60_pulse_1e5
* bench.group: performance
* bench.title: RC ladder with 60 stages, pulse drive, 1e5 steps
* bench.probes: v(n1), v(n30), v(n60), i(v1)
* bench.tol.norm: 2e-3
* bench.expect: match
* bench.ref: reltol=1e-6 abstol=1e-15 vntol=1e-9
* bench.ref.tmax: 4n
* bench.ng.solver: both
* bench.timing: yes
* bench.size: 62
* bench.est_rss_mib: 90
* bench.tags: linear, tran, long, ladder, wide_output, beyond_jit
* bench.note: 62 unknowns, above the JIT size limit: the linear transient runs on the static-pivot LU tier, 63 vectors per row
V1 in 0 PULSE(0 1 20n 2u 2u 100u 200u)
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
R31 n30 n31 1k
C31 n31 0 100p
R32 n31 n32 1k
C32 n32 0 100p
R33 n32 n33 1k
C33 n33 0 100p
R34 n33 n34 1k
C34 n34 0 100p
R35 n34 n35 1k
C35 n35 0 100p
R36 n35 n36 1k
C36 n36 0 100p
R37 n36 n37 1k
C37 n37 0 100p
R38 n37 n38 1k
C38 n38 0 100p
R39 n38 n39 1k
C39 n39 0 100p
R40 n39 n40 1k
C40 n40 0 100p
R41 n40 n41 1k
C41 n41 0 100p
R42 n41 n42 1k
C42 n42 0 100p
R43 n42 n43 1k
C43 n43 0 100p
R44 n43 n44 1k
C44 n44 0 100p
R45 n44 n45 1k
C45 n45 0 100p
R46 n45 n46 1k
C46 n46 0 100p
R47 n46 n47 1k
C47 n47 0 100p
R48 n47 n48 1k
C48 n48 0 100p
R49 n48 n49 1k
C49 n49 0 100p
R50 n49 n50 1k
C50 n50 0 100p
R51 n50 n51 1k
C51 n51 0 100p
R52 n51 n52 1k
C52 n52 0 100p
R53 n52 n53 1k
C53 n53 0 100p
R54 n53 n54 1k
C54 n54 0 100p
R55 n54 n55 1k
C55 n55 0 100p
R56 n55 n56 1k
C56 n56 0 100p
R57 n56 n57 1k
C57 n57 0 100p
R58 n57 n58 1k
C58 n58 0 100p
R59 n58 n59 1k
C59 n59 0 100p
R60 n59 n60 1k
C60 n60 0 100p
.tran 4n 400u
.end
