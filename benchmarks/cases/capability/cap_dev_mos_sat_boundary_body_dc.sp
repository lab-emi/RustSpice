NMOS triode/saturation boundary with body effect (Vgs=2 V, Vsb=1 V, boundary at Vds=1.0609 V)
* bench.id: cap_dev_mos_sat_boundary_body_dc
* bench.group: capability
* bench.title: NMOS triode/saturation boundary with body effect (Vgs=2 V, Vsb=1 V, boundary at Vds=1.0609 V)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: textbook, dc, nmos, body_effect, region_boundary, hand_checked
* bench.note: new deck: Vth(Vsb=1)=0.7+0.5*(sqrt(1.65)-sqrt(0.65))=0.9391 V so the region switch sits at Vds=Vgs-Vth=1.0609 V (not 1.3 V); a 5 mV grid from 0.95 to 1.15 V straddles it; closed-form level-1 current (triode and saturation branches both with 1+LAMBDA*Vds) equals tight ngspice to 2.2e-12 A and RustSpice to 1.2e-12 A on all 41 points
VDS d 0 0.95
VG g 0 2
VB b 0 -1
M1 d g 0 b NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.05 GAMMA=0.5 PHI=0.65)
.dc VDS 0.95 1.15 0.005
.end
