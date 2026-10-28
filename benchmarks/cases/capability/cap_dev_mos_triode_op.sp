NMOS triode OP: Vgs=3 V, Vds=0.1 V (hand check)
* bench.id: cap_dev_mos_triode_op
* bench.group: capability
* bench.title: NMOS triode OP: Vgs=3 V, Vds=0.1 V (hand check)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, op, hand_checked
* bench.note: KP*W/L*((Vgs-VTO)*Vds-Vds^2/2) = 1e-3*(2.3*0.1-0.005) = 225 uA
VDS d 0 0.1
VG g 0 3
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u)
.op
.end
