Diode network with separate models: adjoint fixture (R1 R2 R3 D1.is D2.is)
* bench.id: cap_cli_adj_diode_net
* bench.group: capability
* bench.title: Diode network with separate models, adjoint fixture (R1 R2 R3 D1.is D2.is)
* bench.probes: v(b), v(c), v(d), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-10
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.est_rss_mib: 40
* bench.tags: diode, op, nonlinear, adjoint_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_adjoint_fd.py: `rustspice adjoint --target-node c|d` on this deck is compared with an independent 50-digit mpmath implicit-function oracle and with a Richardson central finite difference of tightly converged ngspice; every one of the 5 parameters moves V(c) by >= 4e-3 V per 100 % change (non-vacuous). The deck itself is an ordinary OP comparison.
V1 a 0 DC 5
R1 a b 1k
D1 b c DM1
R2 c 0 1k
D2 c d DM2
R3 d 0 3k
.model DM1 D(IS=1e-14 N=1)
.model DM2 D(IS=2e-14 N=1.2)
.op
.end
