NMOS L1 source series resistance RS=200 ohm
* bench.id: cap_dev_mos_rs
* bench.group: capability
* bench.title: NMOS L1 source series resistance RS=200 ohm
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: RS ignored: 225 uA at Vds=0.1 V (ngspice 154 uA)
* bench.tags: mosfet, level1, dc
* bench.note: evidence: RustSpice output is bit-identical with and without RS; own level-1 solve with the 200 ohm source resistor equals tight ngspice to 5e-12 A (relative 3e-9) and RustSpice equals the same solve without RS
VDS d 0 0
VG g 0 3
M1 d g 0 0 NM W=10u L=1u
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u RS=200)
.dc VDS 0 5 0.1
.end
