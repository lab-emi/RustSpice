Mutual inductance K between two inductors (transformer, k=0.9)
* bench.id: cap_el_k_mutual_coupling
* bench.group: capability
* bench.title: Mutual inductance K between two inductors (transformer, k=0.9)
* bench.probes: v(sec)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the K card is refused at parse time (`Unsupported device card K1`, exit 1, no panic) where ngspice couples the windings
* bench.tags: coupled, tran, unsupported
* bench.note: The secondary peaks at +0.87 V and -0.96 V for a 1 V sine drive. Independent check: the coupled-inductor ODE [L1 M; M L2] di/dt = [vin - R1*i1; -R2*i2] with M = 0.9*sqrt(L1*L2) integrated with DOP853 (rtol 1e-11) agrees with ngspice-47 (reltol 1e-6, tmax 0.5n) to 2.5e-5 V (1.4e-5 of the 1.83 V swing). Loud refusal of a supported feature is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 SIN(0 1 1meg)
R1 in p 1
L1 p 0 1u
L2 sec 0 1u
K1 L1 L2 0.9
R2 sec 0 1k
.tran 10n 3u
.end
