Series resistor ladder with V and I sources, inductor, capacitor and a diode: adjoint fixture with 20 CLI parameters
* bench.id: cap_cli_adj_wide_ladder_sources
* bench.group: capability
* bench.title: 19-resistor ladder with V and I sources, inductor, capacitor and a diode, adjoint fixture with 20 CLI parameters
* bench.probes: v(n9), v(m), v(k), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-10
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.est_rss_mib: 40
* bench.tags: diode, op, nonlinear, ladder, inductor, capacitor, adjoint_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_adjoint_fd.py: the adjoint CLI enumerates 19 R + 1 D = 20 parameters, prints only the first 15 and never lists V1/I1 although dV(m)/dV1 = 0.062 and dV(m)/dI1 = 218 ohm (independent oracle). The inductor is a DC short, the capacitor an open circuit. The deck itself is an ordinary OP comparison.
V1 n0 0 DC 5
I1 0 m DC 1m
R1 n0 n1 110
R2 n1 n2 120
R3 n2 n3 130
R4 n3 n4 140
R5 n4 n5 150
R6 n5 n6 160
R7 n6 n7 170
R8 n7 n8 180
R9 n8 n9 190
R10 n9 n10 200
R11 n10 n11 210
R12 n11 n12 220
R13 n12 n13 230
R14 n13 n14 240
R15 n14 n15 250
R16 n15 n16 260
R17 n16 n17 270
R18 n17 n18 280
L1 n18 m 10u
C1 m 0 1n
D1 m k DM
R19 k 0 220
.model DM D(IS=2e-14 N=1.05)
.op
.end
