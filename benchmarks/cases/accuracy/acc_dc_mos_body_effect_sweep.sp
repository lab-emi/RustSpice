NMOS drain current versus source potential: source swept 0 .. 2 V with bulk grounded, threshold moves with gamma 0.4
* bench.id: acc_dc_mos_body_effect_sweep
* bench.group: accuracy
* bench.title: NMOS drain current versus source potential: source swept 0 .. 2 V with bulk grounded, threshold moves with gamma 0.4
* bench.probes: i(vd)
* bench.tol.abs: 1e-10
* bench.size: 21
* bench.tags: mos, level1, dc, sweep, body_effect
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vd d 0 3.3
Vg g 0 2.5
Vs s 0 0
M1 d g s 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vs 0 2 0.1
.end
