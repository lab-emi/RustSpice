NMOS source follower, 5k source resistor, body at ground: threshold raised by the body effect (gamma 0.4)
* bench.id: acc_dc_mos_source_follower_body
* bench.group: accuracy
* bench.title: NMOS source follower, 5k source resistor, body at ground: threshold raised by the body effect (gamma 0.4)
* bench.probes: v(s), i(vdd)
* bench.tol.abs: 1e-10
* bench.tags: mos, level1, op, body_effect
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vg g 0 2.0
M1 vdd g s 0 nm w=20u l=1u
Rs s 0 5k
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.op
.end
