CMOS inverter in its steep transition driving an NMOS source follower: adjoint fixture
* bench.id: cap_cli_adj_mos_inv_follower
* bench.group: capability
* bench.title: CMOS inverter on its steep transition driving an NMOS source follower (body effect), adjoint fixture
* bench.probes: v(x), v(s), i(v1)
* bench.expect: match
* bench.tol.rel: 1e-7
* bench.tol.abs: 1e-11
* bench.est_rss_mib: 40
* bench.tags: mos, level1, op, nonlinear, body_effect, high_gain, adjoint_fixture, cli_surface
* bench.note: fixture of checks/untested_cli_surface/check_adjoint_fd.py (adjoint wrt R1.r and W/VTO of three MOSFETs with separate model cards: both inverter devices saturated, follower body-biased, |gain| ~ 90 so every sensitivity is O(1e5 V/m) or O(10 V/V)). Probe set includes the internal gain node x. The deck itself is an ordinary OP comparison.
V1 vdd 0 DC 3.3
V2 in 0 DC 1.5125
M1 x in 0 0 NM W=10u L=1u
M2 x in vdd vdd PM W=20u L=1u
M3 vdd x s 0 NF W=40u L=1u
R1 s 0 20k
.model NM NMOS(LEVEL=1 VTO=0.7 KP=120u LAMBDA=0.04 GAMMA=0.4 PHI=0.7)
.model PM PMOS(LEVEL=1 VTO=-0.8 KP=40u LAMBDA=0.05 GAMMA=0.5 PHI=0.7)
.model NF NMOS(LEVEL=1 VTO=0.75 KP=110u LAMBDA=0.03 GAMMA=0.45 PHI=0.65)
.op
.end
