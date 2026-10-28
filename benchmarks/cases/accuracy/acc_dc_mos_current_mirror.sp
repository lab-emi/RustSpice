NMOS current mirror, 20 uA reference, 1:2 ratio, output held at 1.5 V (channel-length modulation visible)
* bench.id: acc_dc_mos_current_mirror
* bench.group: accuracy
* bench.title: NMOS current mirror, 20 uA reference, 1:2 ratio, output held at 1.5 V (channel-length modulation visible)
* bench.probes: v(g), i(vo), i(vdd)
* bench.tol.abs: 1e-10
* bench.tags: mos, level1, op, mirror
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Iref vdd g 20u
M1 g g 0 0 nm w=10u l=1u
M2 o g 0 0 nm w=20u l=1u
Vo o 0 1.5
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.op
.end
