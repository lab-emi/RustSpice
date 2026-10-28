NMOS id-vds at Vgs = 2 V (Vov = 1.3 V), Vds swept 0 .. 3.3 V across the triode/saturation boundary at 1.3 V
* bench.id: acc_dc_mos_idvds_nmos_boundary
* bench.group: accuracy
* bench.title: NMOS id-vds at Vgs = 2 V (Vov = 1.3 V), Vds swept 0 .. 3.3 V across the triode/saturation boundary at 1.3 V
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.size: 34
* bench.tags: mos, level1, dc, sweep, region_boundary
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance; the sweep grid contains Vds = 1.3 V exactly
Vgs g 0 2.0
Vds d 0 0
M1 d g 0 0 nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vds 0 3.3 0.1
.end
