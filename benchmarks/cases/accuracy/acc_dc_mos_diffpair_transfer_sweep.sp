NMOS differential pair transfer curve: Vp swept 0.6 -> 1.8 V in 10 mV steps around Vn = 1.2 V, both devices pass through cut-off
* bench.id: acc_dc_mos_diffpair_transfer_sweep
* bench.group: accuracy
* bench.title: NMOS differential pair transfer curve: Vp swept 0.6 -> 1.8 V in 10 mV steps around Vn = 1.2 V, both devices pass through cut-off
* bench.probes: v(o1), v(o2), v(tail)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.size: 121
* bench.tags: mos, level1, dc, sweep, diffpair, isrc, cutoff, robustness
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance; every point is also solved by RS as a stand-alone .op. Slug linear-static-pivot-accuracy (read as covering nonlinear Newton), not dc-sweep-no-recovery: same stall signature as the two-point deck (a noise floor of the frozen static-pivot LU above abstol, not a missing sweep recovery); faer LU alone and abstol=1e-9 alone each close it (reproduced, gap_closed under the faer LU).
Vdd vdd 0 3.3
Vp p 0 0.6
Vn n 0 1.2
Itail tail 0 100u
Rl1 vdd o1 10k
Rl2 vdd o2 10k
M1 o1 p tail 0 nm w=20u l=1u
M2 o2 n tail 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vp 0.6 1.8 0.01
.end
