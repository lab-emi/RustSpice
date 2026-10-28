MOSFET with a valid sub-picometre channel length L=0.5p
* bench.id: rob_hs_mos_l_subpico
* bench.group: robustness
* bench.title: MOSFET with a valid sub-picometre channel length L=0.5p (W=10n, KP=100u)
* bench.probes: i(vd)
* bench.expect: match
* bench.tags: hostile, nonlinear, mos, value, silent_risk
* bench.est_rss_mib: 64
* bench.note: oracle = closed form, independent of both simulators: ID = 0.5*KP*(W/L)*(VGS-VTO)^2 = 0.5*1e-4*(10n/0.5p)*1.3^2 = 1.69 A (i(vd) = -1.69 A); ngspice with reltol=1e-9 gives -1.69 A. Variants (same card with W=10u, both tools run): L=1p, 2p and 1n agree between RustSpice, ngspice and the closed form (-845, -422.5 and -0.845 A); below 1p RustSpice returns the L=1p value for every L (0.5p, 0.1p, 1f), ngspice and the closed form scale as 1/L (-1690, -8450, -845000 A). The L=0 card is rob_hs_mos_l_zero (loud_error). Companion of cap_dev_mos_phi_small (the same floor mechanism on PHI)
VD d 0 3
VG g 0 2
M1 d g 0 0 NM W=10n L=0.5p
.model NM NMOS(VTO=0.7 KP=100u)
.op
.end
