NMOS saturation OP: Vgs=2 V, Vds=3 V, LAMBDA=0.02 (hand check)
* bench.id: cap_dev_mos_sat_op
* bench.group: capability
* bench.title: NMOS saturation OP: Vgs=2 V, Vds=3 V, LAMBDA=0.02 (hand check)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, op, hand_checked
* bench.note: 0.5*KP*W/L*(Vgs-VTO)^2*(1+LAMBDA*Vds) = 0.5e-3*1.69*1.06 = 895.7 uA
VDS d 0 3
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.op
.end
