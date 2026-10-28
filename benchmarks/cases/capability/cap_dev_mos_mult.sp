NMOS L1 instance multiplier M=4
* bench.id: cap_dev_mos_mult
* bench.group: capability
* bench.title: NMOS L1 instance multiplier M=4
* bench.probes: i(vds)
* bench.tol.abs: 1e-10
* bench.expect: match
* bench.known_gap: silent-ignore-multiplier
* bench.gap.note: M=4 ignored: 0.93 mA at Vds=5 V (ngspice 3.72 mA)
* bench.tags: mosfet, level1, dc
* bench.note: evidence: RustSpice output is bit-identical with and without M=4; tight ngspice is exactly 4.0x (3.72 mA vs 0.93 mA at Vds=5 V). Independent closed form (saturation) 4*0.5*KP*(W/L)*(Vgs-VTO)^2*(1+LAMBDA*Vds) = 3.718 mA equals tight ngspice. Only M is dropped: RustSpice with W=40u (M folded into W) gives 3.718 mA.
VDS d 0 0
VG g 0 2
M1 d g 0 0 NM W=10u L=1u M=4
.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u LAMBDA=0.02)
.dc VDS 0 5 0.1
.end
