2x3 RC mesh with a load: .tran 10p 100n ends on a 1e-20 s sliver step
* bench.id: acc_tr_last_step_rc_mesh
* bench.group: accuracy
* bench.title: 2x3 RC mesh (1k links, 1p nodes, 2k load) with .tran 10p 100n: the final row is written by a 1e-20 s sliver step
* bench.probes: v(n0_0), v(n0_2), v(n1_1), v(n1_2), i(v1)
* bench.tol.norm: 5e-3
* bench.oracle: tran_lti
* bench.expect: match
* bench.tags: linear, tran, mesh, final_time, npoints
* bench.est_rss_mib: 32
* bench.note: oracle tran_lti (dense f64) and converged ngspice agree to 3e-6 of swing; flip test: the same mesh with .tran 20p 100n or 12.5p writes the expected rows and no garbage; a 2x2 mesh, a 3x2 mesh and every RC ladder up to 60 stages show the extra row with a benign value, the 3x3 mesh reaches 3e6 V. The sliver system itself is not ill-posed: C/h is 8.7e7 S, its matrix has cond 8.8e7, and a dense f64 partial-pivot solve of that step (state of the 10001st row) returns the previous row to 1.1e-16 V, so the 4e5 V comes from RustSpice's linear solver on a stiff but well-posed system (the family of linear-static-pivot-accuracy), which the grid defect merely exposes; this deck gates the extra row (npoints).
V1 in 0 PULSE(0 1 10n 10n 10n 200n 400n)
Rin in n0_0 1k
C0_0 n0_0 0 1p
Rh0_0 n0_0 n0_1 1k
Rv0_0 n0_0 n1_0 1k
C0_1 n0_1 0 1p
Rh0_1 n0_1 n0_2 1k
Rv0_1 n0_1 n1_1 1k
C0_2 n0_2 0 1p
Rv0_2 n0_2 n1_2 1k
C1_0 n1_0 0 1p
Rh1_0 n1_0 n1_1 1k
C1_1 n1_1 0 1p
Rh1_1 n1_1 n1_2 1k
C1_2 n1_2 0 1p
Rload n1_2 0 2k
.tran 10p 100n
.end
