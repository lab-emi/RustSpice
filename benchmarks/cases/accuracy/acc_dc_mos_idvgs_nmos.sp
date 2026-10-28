NMOS id-vgs transfer curve at Vds = 0.1 V (triode), Vgs swept 0 .. 3.3 V in 50 mV steps through threshold
* bench.id: acc_dc_mos_idvgs_nmos
* bench.group: accuracy
* bench.title: NMOS id-vgs transfer curve at Vds = 0.1 V (triode), Vgs swept 0 .. 3.3 V in 50 mV steps through threshold
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.size: 67
* bench.tags: mos, level1, dc, sweep, triode
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vds d 0 0.1
Vgs g 0 0
M1 d g 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vgs 0 3.3 0.05
.end
