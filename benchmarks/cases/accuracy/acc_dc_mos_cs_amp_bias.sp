NMOS common-source stage, 10k load, Vg = 1.0 V: saturation bias point
* bench.id: acc_dc_mos_cs_amp_bias
* bench.group: accuracy
* bench.title: NMOS common-source stage, 10k load, Vg = 1.0 V: saturation bias point
* bench.probes: v(d), i(vdd)
* bench.tol.abs: 1e-10
* bench.tags: mos, level1, op
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vg g 0 1.0
Rd vdd d 10k
M1 d g 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.op
.end
