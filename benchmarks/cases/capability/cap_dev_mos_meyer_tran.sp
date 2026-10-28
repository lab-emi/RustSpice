NMOS Meyer gate capacitance from TOX=10 nm (no overlap caps), W=50u L=2u
* bench.id: cap_dev_mos_meyer_tran
* bench.group: capability
* bench.title: NMOS Meyer gate capacitance from TOX=10 nm (no overlap caps), W=50u L=2u
* bench.probes: v(g), v(d)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: TOX ignored, no Meyer gate capacitance: RS equals the no-gate-capacitance solution (2.2e-9 normalised), norm err 0.67 (v(g)) and 0.98 (v(d)) against tol 5e-3
* bench.tags: mosfet, level1, tran, capacitance
* bench.note: evidence: RustSpice output is bit-identical with and without TOX. Independent oracle: the textbook Meyer capacitances (Cgs/Cgd/Cgb from Cox*W*L = 345.3 fF, VTO = 0.7, default PHI = 0.6) coupled with the gate RC and the algebraic level-1 drain as a 2-state ODE (python Radau) agree with tight ngspice to 1.1e-6 (v(g)) and 7.9e-5 (v(d)) normalised and miss RustSpice by 0.67 / 0.98; ngspice without TOX equals RustSpice (1.5e-16 / 1.8e-5). A lumped explicit Cg = Cox*W*L added to RustSpice is NOT a faithful flip (0.13 / 0.50 from ngspice) because the Meyer values change with bias. ngspice as-is is 7.4e-4 / 2.0e-3 from its converged reference
VDD vdd 0 3
RD vdd d 2k
VIN in 0 PULSE(0 3 5n 2n 2n 100n 200n)
RG in g 10k
M1 d g 0 0 NM W=50u L=2u

.model NM NMOS(LEVEL=1 VTO=0.7 KP=100u TOX=1e-8)
.tran 0.05n 40n
.end
