NMOS L1 small valid surface potential PHI=1m with GAMMA=0.5 and VBS=-1
* bench.id: cap_dev_mos_phi_small
* bench.group: capability
* bench.title: NMOS L1 small valid surface potential PHI=1m with GAMMA=0.5 and VBS=-1
* bench.probes: i(vds)
* bench.expect: match
* bench.tags: mosfet, level1, model_card, op, silent_risk
* bench.note: oracle = closed form, independent of both simulators: VTH = VTO + GAMMA*(sqrt(PHI-VBS) - sqrt(PHI)) = 0.7 + 0.5*(sqrt(1.001) - sqrt(0.001)) = 1.18439 V, ID = 0.5*KP*W/L*(VGS-VTH)^2 = 0.5*1e-3*0.81561^2 = 3.3257e-4 A (flows from the + terminal of VDS through the source: i(vds) = -3.3257e-4 A); ngspice with reltol=1e-9 gives -3.325702e-4 A. Variants: RustSpice returns the same -4.359067e-4 A for PHI=0.001, 0.01, 0.05 and 0.1 (ngspice: -3.3257e-4, -3.5913e-4, -4.0451e-4, -4.3591e-4 A) and agrees with ngspice at PHI=0.11 (-4.4091e-4 A, to 3e-9 relative); the same card with PHI=0 is cap_dev_mos_phi_nonpositive (loud_error). Companion of rob_hs_mos_l_subpico (the same floor mechanism on L)
VDS d 0 3
VG g 0 2
VB b 0 -1
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u GAMMA=0.5 PHI=1m)
.op
.end
