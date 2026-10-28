Diode forward-bias capacitance coefficient FC=0.1 (VJ=0.8 V, CJO=2 pF, IS=1e-16)
* bench.id: cap_dev_dio_fc
* bench.group: capability
* bench.title: Diode forward-bias capacitance coefficient FC=0.1 (VJ=0.8 V, CJO=2 pF, IS=1e-16)
* bench.probes: v(a)
* bench.expect: match
* bench.tags: diode, tran, capacitance, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models graded junction capacitance CJO/VJ/M/FC; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without FC=0.1, but the VJ/M grading it also ignores dominates (RustSpice is 0.137 V = 0.186 of the swing off ngspice even with FC at its default; FC alone moves ngspice by 0.043 V = 5.9% of the 0.736 V swing, 11.8x tol.norm); own ODE solution with the SPICE forward-bias capacitance polynomial equals the converged ngspice reference to 8e-6 normalised (ngspice as-is 2.5e-3), RustSpice 0.143
V1 in 0 PULSE(0 3 5n 5n 5n 800n 1600n)
R1 in a 10k
D1 a 0 DM
.model DM D(IS=1e-16 CJO=2p VJ=0.8 FC=0.1)
.tran 0.5n 100n
.end
