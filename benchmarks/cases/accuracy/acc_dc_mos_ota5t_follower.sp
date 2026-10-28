Five-transistor OTA connected as a unity-gain follower (bias mirror, tail, pair, mirror load)
* bench.id: acc_dc_mos_ota5t_follower
* bench.group: accuracy
* bench.title: Five-transistor OTA connected as a unity-gain follower (bias mirror, tail, pair, mirror load)
* bench.probes: v(out), v(d1), v(tail), v(nb), i(vdd)
* bench.tol.abs: 1e-10
* bench.size: 9
* bench.tags: mos, level1, op, ota, feedback
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vin inp 0 1.2
Ib 0 nb 20u
Mb nb nb 0 0 nm w=4u l=1u
Mtail tail nb 0 0 nm w=8u l=1u
M1 d1 inp tail 0 nm w=20u l=1u
M2 out out tail 0 nm w=20u l=1u
M3 d1 d1 vdd vdd pm w=40u l=1u
M4 out d1 vdd vdd pm w=40u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.op
.end
