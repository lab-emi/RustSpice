NMOS L1 TOX=10 nm only (KP from default UO=600)
* bench.id: cap_dev_mos_tox_only
* bench.group: capability
* bench.title: NMOS L1 TOX=10 nm only (KP from default UO=600)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
* bench.note: Gap closed 2026-10-28: RustSpice now models KP = UO Cox from TOX; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without TOX; hand: KP=UO*Cox=600e-4*3.9*8.854e-12/1e-8=2.07e-4 A/V^2 gives 1.926 mA at Vds=5 V = tight ngspice. Closed-form level-1 with CODATA-2018 eps0 differs from tight ngspice by 5.9e-9 A at the worst point (ngspice's effective eps0 is 3.06e-6 above CODATA; RustSpice with an explicit KP computed from ngspice's constant matches it to 2e-14 A), so a RustSpice using the CODATA constant would stay over tolerance (tol.abs 1e-10, tol.rel 1e-6) at 50 of 51 points and the deck can only flip to gap_closed with ngspice's constant. Same root cause as acc_dc_mos_uo_tox_transconductance, cap_dev_mos_u0 and cap_dev_mos_uo_tox
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 TOX=1e-8 LAMBDA=0.02)
.dc VDS 0 5 0.1
.end
