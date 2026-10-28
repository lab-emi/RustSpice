NMOS drain sidewall junction CJSW=1 nF/m with PD=2000 um (2 pF zero-bias)
* bench.id: cap_dev_mos_cjsw_tran
* bench.group: capability
* bench.title: NMOS drain sidewall junction CJSW=1 nF/m with PD=2000 um (2 pF zero-bias)
* bench.probes: v(d)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: no sidewall junction capacitance (CJSW/PD/PS/MJSW ignored): RS equals the no-capacitance solution, norm err 0.77 against tol 5e-3
* bench.ref.tmax: 5p
* bench.tags: mosfet, level1, tran, capacitance
* bench.note: ref.tmax 5p: with tmax = 5 ps tight ngspice finishes in under 0.1 s (about 8000 points) while with tmax = 2 ps, 1 ps and 0.5 ps it did not finish within 40 s on this junction-capacitance deck; evidence: RustSpice output is bit-identical with and without CJSW/PD/PS/MJSW/PB and equals the no-capacitance algebraic solution to 2.2e-9 normalised; independent ODE (python Radau, C = CJSW*PD/(1+Vd/PB)^MJSW, 2 pF at zero bias) agrees with the converged ngspice reference to 2.4e-6 normalised (ngspice as-is 9.7e-4 from the ODE on its own time grid and 2.4e-3 from the reference in the engine's resampled metric), RustSpice misses it by 0.77
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=50u L=2u PD=2000u PS=2000u

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u CJSW=1n MJSW=0.33 PB=0.8)
.tran 0.1n 40n
.end
