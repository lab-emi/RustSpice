NMOS L1 mobility given as U0=600 with TOX=10 nm
* bench.id: cap_dev_mos_u0
* bench.group: capability
* bench.title: NMOS L1 mobility given as U0=600 with TOX=10 nm
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
* bench.note: Gap closed 2026-10-28: RustSpice now models KP = UO Cox from TOX; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without U0/TOX; hand: KP=U0*Cox=2.07e-4 A/V^2, triode Id=KP*W/L*(Vov*Vds-Vds^2/2)=17.41 mA at Vgs=5 V = tight ngspice 17.40 mA. With CODATA-2018 eps0 the closed form is 5.3e-8 A from tight ngspice at the worst point against a tolerance of 1.7e-8 A (ngspice's effective eps0 is 3.06e-6 above CODATA), so the deck can only flip to gap_closed with ngspice's constant. U0 is the common alias of UO and ngspice honours it
VDS d 0 3
VG g 0 0
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 U0=600 TOX=1e-8)
.dc VG 0 5 0.1
.end
