NMOS CGBO=50 nF/m gate-bulk overlap (scales with L, W=50u L=2u)
* bench.id: cap_dev_mos_cgbo_tran
* bench.group: capability
* bench.title: NMOS CGBO=50 nF/m gate-bulk overlap (scales with L, W=50u L=2u)
* bench.probes: v(g), v(d)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: CGBO is applied per W (2.5 pF) instead of per L (0.1 pF): v(g) reaches 2.23 V at 40 ns (ngspice 3.00 V), norm err 0.83 (v(g)) and 1.00 (v(d)) against tol 5e-3
* bench.tags: mosfet, level1, tran, capacitance
* bench.note: evidence: independent ODE (python Radau: gate RC with C = CGBO*L = 0.1 pF plus the algebraic level-1 drain) agrees with tight ngspice to 1.2e-7 (v(g)) and 8.2e-6 (v(d)) normalised and misses RustSpice by 0.83 / 1.00; with C = CGBO*W = 2.5 pF the same ODE matches RustSpice to 4.1e-7 / 2.8e-7 and misses ngspice by 1.12 / 1.05. RustSpice does not react to L (L 2u -> 4u: v(g) at 40 ns stays 2.22981 V), W 50u -> 100u moves it to 1.480 V, W = L = 2u (CGBO*W = CGBO*L) closes it (3.8e-5 / 5.4e-5 against tight ngspice) and without CGBO RustSpice equals ngspice (1.5e-16 / 1.8e-5) while differing from its own CGBO run by 2.9 V. ngspice as-is is 5.0e-3 (v(d)) from its converged reference, just under tol.norm 5e-3, so the as-is ngspice run is itself marginal; RustSpice's error is 200x the tolerance and the verdict does not depend on it
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=50u L=2u

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u CGBO=50n)
.tran 0.05n 40n
.end
