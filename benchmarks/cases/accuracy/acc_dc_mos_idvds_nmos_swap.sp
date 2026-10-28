NMOS id-vds with Vds swept -2 V .. +2 V and the bulk at -2.5 V: source/drain exchange below Vds = 0
* bench.id: acc_dc_mos_idvds_nmos_swap
* bench.group: accuracy
* bench.title: NMOS id-vds with Vds swept -2 V .. +2 V and the bulk at -2.5 V: source/drain exchange below Vds = 0
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.size: 41
* bench.tags: mos, level1, dc, sweep, source_drain_swap, body_effect
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance; bulk at -2.5 V keeps both bulk junctions reverse biased over the whole sweep
Vgs g 0 2.0
Vds d 0 0
Vb b 0 -2.5
M1 d g 0 b nm w=20u l=1u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.dc Vds -2 2 0.1
.end
