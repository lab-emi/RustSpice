Diode forward bias up to 1 A with RS=50 mohm, current-driven
* bench.id: cap_dev_dio_fwd_1a_rs
* bench.group: capability
* bench.title: Diode forward bias up to 1 A with RS=50 mohm, current-driven
* bench.probes: v(a)
* bench.tol.rel: 1e-5
* bench.expect: match
* bench.tags: diode, dc, high_current, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models diode series resistance RS (internal node); the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without RS; hand solution v=VT*ln(I/IS+1)+I*RS equals tight ngspice to 2.8e-7 V (physical constants) and RustSpice equals the same formula without RS; RustSpice error 0.04755 V = I*RS at 0.951 A (an explicit 0.05 ohm resistor in RustSpice matches tight ngspice to 6e-10 V)
I1 0 a 1m
D1 a 0 DM
.model DM D(IS=1e-14 RS=0.05)
.dc I1 1m 1 0.05
.end
