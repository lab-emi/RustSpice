NMOS L1 lateral diffusion LD=0.1u (Leff=0.8u)
* bench.id: cap_dev_mos_ld
* bench.group: capability
* bench.title: NMOS L1 lateral diffusion LD=0.1u (Leff=0.8u)
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
* bench.note: Gap closed 2026-10-28: RustSpice now models lateral diffusion LD; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without LD; own level-1 solve with Leff=L-2*LD=0.8u equals tight ngspice to 5e-12 A and RustSpice equals the same solve at L=1u to 5e-12 A
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02 LD=0.1u)
.dc VDS 0 5 0.1
.end
