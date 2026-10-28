Transmission gate with the NMOS drain and source labels swapped: adjoint fixture
* bench.id: cap_cli_adj_mos_tgate
* bench.group: capability
* bench.title: Transmission gate, NMOS drain/source swapped (reversed-mode branch) and PMOS in triode, adjoint fixture
* bench.probes: v(x), v(y), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-7
* bench.tol.abs: 1e-11
* bench.est_rss_mib: 40
* bench.tags: mos, level1, op, nonlinear, reversed_mode, triode, pmos, adjoint_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_adjoint_fd.py: M1 is wired y(drain)-gate-x(source) while the current flows x to y, so the Level-1 evaluation takes the reversed (vds<0) branch; M2 is a PMOS in triode. The deck itself is an ordinary OP comparison.
V1 vin 0 DC 2.2
V2 gn 0 DC 3.3
V3 gp 0 DC 0
R1 vin x 500
M1 y gn x 0 TN W=8u L=1u
M2 y gp x x TP W=16u L=1u
R2 y 0 4k
.model TN NMOS(LEVEL=1 VTO=0.6 KP=90u LAMBDA=0.03 GAMMA=0.35 PHI=0.7)
.model TP PMOS(LEVEL=1 VTO=-0.7 KP=35u LAMBDA=0.04 GAMMA=0.4 PHI=0.7)
.op
.end
