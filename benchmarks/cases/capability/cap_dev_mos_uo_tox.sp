NMOS L1 KP derived from UO=600 cm2/Vs and TOX=10 nm
* bench.id: cap_dev_mos_uo_tox
* bench.group: capability
* bench.title: NMOS L1 KP derived from UO=600 cm2/Vs and TOX=10 nm
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
* bench.note: Gap closed 2026-10-28: RustSpice now models KP = UO Cox from TOX; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice reads UO as KP (UO=1 is bit-identical to KP=1, UO=0.001 to KP=0.001), so UO=600 becomes KP=600 A/V^2 and Newton fails: RustSpice refuses loudly but for the wrong reason, which is why it is not an unsupported-*-loud gap; ngspice derives KP=UO*Cox=2.07e-4 A/V^2 (hand-checked on cap_dev_mos_tox_only, including its eps0 caveat: the deck can only flip to gap_closed with ngspice's constant). Controls: an explicit KP=600 gives the identical RustSpice failure while ngspice converges there (i(vds) = -5577 A at Vds=5 V), so RustSpice also has a latent Newton-robustness weakness at beta ~ 1e4 that this deck only exposes; an explicit KP=2.0718863e-4 next to UO and TOX (KP wins) makes RustSpice match ngspice to 2e-11 A
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 UO=600 TOX=1e-8 LAMBDA=0.02)
.dc VDS 0 5 0.1
.end
