MOSFET with L=0 (division by zero in beta)
* bench.id: rob_hs_mos_l_zero
* bench.group: robustness
* bench.title: MOSFET with L=0 (division by zero in beta)
* bench.probes: v(d), i(vd)
* bench.expect: loud_error
* bench.tags: hostile, nonlinear, mos, value, div_zero, silent_risk
* bench.est_rss_mib: 64
* bench.note: a zero channel length makes beta = KP*W/L infinite, there is no physical or numerical answer, so refusing is correct. The mechanism is an evaluator floor on L, not a missing validation: RustSpice gives i(vd) = -845.000000000003 A for L=0, L=-1u, L=1p, 0.5p, 0.1p and 1f alike (= 8.45e-10/L at L=1e-12), so valid L below 1p is wrong too (L=0.5p: RustSpice -845 A, ngspice and the closed form -1690 A for W=10u; see rob_hs_mos_l_subpico, expect match). ngspice does not validate L either: for L=0 it stops with 'Timestep too small; trouble with nm-instance m1' after its gmin/source stepping fails (exit 1, no solution), so its refusal is a non-convergence and not a validation message; for L=-1u it only warns and exits 0 with +8.45e-4 A, so L<0 is NOT refused by ngspice. Retagged from lenient-malformed-input to param-floor-clamp
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=0
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
