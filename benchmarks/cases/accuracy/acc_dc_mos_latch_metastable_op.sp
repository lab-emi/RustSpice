Cross-coupled CMOS inverters without initial conditions: the symmetric metastable operating point
* bench.id: acc_dc_mos_latch_metastable_op
* bench.group: accuracy
* bench.title: Cross-coupled CMOS inverters without initial conditions: the symmetric metastable operating point
* bench.probes: v(q), v(qb)
* bench.tol.abs: 1e-10
* bench.tags: mos, level1, op, bistable, cmos
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance; both simulators find the unstable symmetric equilibrium (1.5548 V)
Vdd vdd 0 3.3
M1 q qb 0 0 nm w=2u l=0.5u
M2 q qb vdd vdd pm w=4u l=0.5u
M3 qb q 0 0 nm w=2u l=0.5u
M4 qb q vdd vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.model pm pmos(level=1 vto=-0.8 kp=40u gamma=0.5 phi=0.7 lambda=0.04)
.op
.end
