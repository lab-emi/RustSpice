NMOS body effect ladder: four devices at Vsb = 0, 1, 2, 3 V (closed-form check)
* bench.id: cap_dev_mos_body_vsb_ladder_op
* bench.group: capability
* bench.title: NMOS body effect ladder: four devices at Vsb = 0, 1, 2, 3 V (closed-form check)
* bench.probes: i(vd0), i(vd1), i(vd2), i(vd3)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: textbook, op, nmos, body_effect, hand_checked
* bench.note: new deck: Vth=VTO+GAMMA*(sqrt(PHI+Vsb)-sqrt(PHI)) gives 0.7000/0.9391/1.1108/1.2521 V; saturation Id=0.5*KP*W/L*(Vgs-Vth)^2*(1+LAMBDA*Vds) = 895.70/596.465/419.032/296.429 uA (closed form), RustSpice and tight ngspice both within 3e-12..6e-12 A of it (gmin leakage terms); i(vb*) is not probed (RustSpice has no bulk junctions, ngspice leaks 3-9 pA there, below the 1e-6 floor)
VG g 0 2
VD0 d0 0 3
VD1 d1 0 3
VD2 d2 0 3
VD3 d3 0 3
VB0 b0 0 0
VB1 b1 0 -1
VB2 b2 0 -2
VB3 b3 0 -3
M0 d0 g 0 b0 NM W=10u L=1u
M1 d1 g 0 b1 NM W=10u L=1u
M2 d2 g 0 b2 NM W=10u L=1u
M3 d3 g 0 b3 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 GAMMA=0.5 PHI=0.65)
.op
.end
