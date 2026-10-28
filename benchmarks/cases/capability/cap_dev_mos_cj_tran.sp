NMOS drain junction CJ=1 mF/m2 with AD=2000 um2 (2 pF zero-bias)
* bench.id: cap_dev_mos_cj_tran
* bench.group: capability
* bench.title: NMOS drain junction CJ=1 mF/m2 with AD=2000 um2 (2 pF zero-bias)
* bench.probes: v(d)
* bench.expect: match
* bench.ref.tmax: 5p
* bench.tags: mosfet, level1, tran, capacitance
* bench.note: Gap closed 2026-10-28: RustSpice now models bulk junction capacitance CJ/MJ/PB; the evidence below records the reference and the behaviour before the fix. ref.tmax 5p: with tmax = 5 ps tight ngspice finishes in under 0.1 s (about 8000 points) while with tmax = 2 ps, 1 ps and 0.5 ps it did not finish within 40 s on this junction-capacitance deck; evidence: RustSpice output is bit-identical with and without CJ/AD/AS/MJ/PB and equals the no-capacitance algebraic solution to 2.2e-9 normalised; independent ODE (python Radau, C = CJ*AD/(1+Vd/PB)^MJ, 2 pF at zero bias) agrees with the converged ngspice reference to 2.4e-6 normalised (ngspice as-is 1.0e-3 from the ODE on its own time grid and 2.6e-3 from the reference in the engine's resampled metric), RustSpice misses it by 0.74
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=50u L=2u AD=2000p AS=2000p

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u CJ=1m MJ=0.5 PB=0.8)
.tran 0.1n 40n
.end
