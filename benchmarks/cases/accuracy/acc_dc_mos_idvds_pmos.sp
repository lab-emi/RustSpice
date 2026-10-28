PMOS id-vds at Vsg = 2.2 V, drain swept 3.3 V .. 0 V (decreasing)
* bench.id: acc_dc_mos_idvds_pmos
* bench.group: accuracy
* bench.title: PMOS id-vds at Vsg = 2.2 V, drain swept 3.3 V .. 0 V (decreasing)
* bench.probes: i(vd)
* bench.tol.abs: 1e-10
* bench.size: 34
* bench.tags: mos, level1, dc, sweep, pmos, sweep_down
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vg g 0 1.1
Vd d 0 3.3
M1 d g vdd vdd pm w=40u l=1u
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.dc Vd 3.3 0 -0.1
.end
