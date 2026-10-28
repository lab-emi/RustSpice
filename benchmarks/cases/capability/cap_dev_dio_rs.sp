Diode series resistance RS=10 ohm, voltage-driven forward I-V
* bench.id: cap_dev_dio_rs
* bench.group: capability
* bench.title: Diode series resistance RS=10 ohm, voltage-driven forward I-V
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models diode series resistance RS (internal node); the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without RS; hand solution Vd=VT*ln(I/IS), I=(2-Vd)/RS gives 0.1221 A = tight ngspice 0.12206 A; without RS the 2 V ideal source sits directly across the diode and RustSpice silently returns IS*exp(2/VT) = 3.8e19 A, the RS=0 Shockley value, not a clamped one (the exponent argument 2/VT = 77.3 is below the clamp at 80; bit-identical without RS); an explicit 10 ohm resistor in RustSpice reproduces ngspice
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-14 RS=10)
.dc V1 0 2 0.1
.end
