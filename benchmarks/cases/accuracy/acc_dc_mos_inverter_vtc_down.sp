CMOS inverter voltage-transfer curve, Vin swept 3.3 -> 0 V in 10 mV steps (transition at 1.60 V)
* bench.id: acc_dc_mos_inverter_vtc_down
* bench.group: accuracy
* bench.title: CMOS inverter voltage-transfer curve, Vin swept 3.3 -> 0 V in 10 mV steps (transition at 1.60 V)
* bench.probes: v(out), i(vdd)
* bench.tol.abs: 1e-10
* bench.size: 331
* bench.tags: mos, level1, dc, sweep, cmos, vtc, sweep_down
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vin in 0 3.3
Mn out in 0 0 nm w=10u l=1u
Mp out in vdd vdd pm w=25u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.dc Vin 3.3 0 -0.01
.end
