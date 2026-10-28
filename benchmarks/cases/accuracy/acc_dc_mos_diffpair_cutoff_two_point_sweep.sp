NMOS differential pair with M1 in cut-off at both points of a two-point sweep: Vp 0.60 -> 0.61 V (nothing changes but the cut-off gate voltage)
* bench.id: acc_dc_mos_diffpair_cutoff_two_point_sweep
* bench.group: accuracy
* bench.title: NMOS differential pair with M1 in cut-off at both points of a two-point sweep: Vp 0.60 -> 0.61 V (nothing changes but the cut-off gate voltage)
* bench.probes: v(o1), v(o2), v(tail)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.size: 2
* bench.tags: mos, level1, dc, sweep, diffpair, isrc, cutoff, robustness, minimal_repro
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance. Slug linear-static-pivot-accuracy (read as covering nonlinear Newton), not dc-sweep-no-recovery: the failing point is trivially easy (the state is unchanged) and the Newton iteration cycles at a ~1e-10 A noise floor of the frozen static-pivot LU; no recovery is added in the flips (faer LU alone and abstol=1e-9 alone each close the deck, reproduced, gap_closed under the faer LU).
Vdd vdd 0 3.3
Vp p 0 0.6
Vn n 0 1.2
Itail tail 0 100u
Rl1 vdd o1 10k
Rl2 vdd o2 10k
M1 o1 p tail 0 nm w=20u l=1u
M2 o2 n tail 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vp 0.6 0.61 0.01
.end
