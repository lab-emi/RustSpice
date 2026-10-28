NMOS cascode stage with 20k load: high output resistance makes the output lambda-sensitive
* bench.id: acc_dc_mos_cascode_bias
* bench.group: accuracy
* bench.title: NMOS cascode stage with 20k load: high output resistance makes the output lambda-sensitive
* bench.probes: v(d), v(x), i(vdd)
* bench.tol.abs: 1e-10
* bench.tags: mos, level1, op, cascode
* bench.note: tol.abs 1e-10 (V and A): default 1e-9 would be 1e-5 relative on 100 uA currents; RS puts gmin across drain-source whereas ngspice puts gmin and IS on the bulk junctions: a few pA of leakage difference, below this tolerance
Vdd vdd 0 3.3
Vb1 g1 0 1.1
Vb2 g2 0 1.9
Rd vdd d 20k
M2 d g2 x 0 nm w=20u l=1u
M1 x g1 0 0 nm w=20u l=2u
.model nm nmos(level=1 vto=0.7 kp=100u gamma=0.4 phi=0.7 lambda=0.03)
.op
.end
