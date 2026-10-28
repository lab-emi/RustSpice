NMOS L1 drain series resistance RD=200 ohm
* bench.id: cap_dev_mos_rd
* bench.group: capability
* bench.title: NMOS L1 drain series resistance RD=200 ohm
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.tags: mosfet, level1, dc
* bench.note: Gap closed 2026-10-28: RustSpice now models drain series resistance RD (internal node); the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without RD; own level-1 solve with the 200 ohm drain resistor equals tight ngspice to 4.5e-12 A (relative 2e-9) and RustSpice equals the same solve without RD
VDS d 0 0
VG g 0 3
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u RD=200)
.dc VDS 0 5 0.1
.end
