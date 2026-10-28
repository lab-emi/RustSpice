Diode grading coefficient M=0.33 with CJO=2 pF
* bench.id: cap_dev_dio_m_grading
* bench.group: capability
* bench.title: Diode grading coefficient M=0.33 with CJO=2 pF
* bench.probes: v(a)
* bench.expect: match
* bench.tags: diode, tran, capacitance, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models graded junction capacitance CJO/VJ/M/FC; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without M=0.33; own ODE solution (python Radau, graded junction capacitance) equals the converged ngspice reference to 4.2e-7 normalised (ngspice as-is 3e-4), RustSpice 0.097
V1 in 0 PULSE(0 -3 5n 5n 5n 400n 800n)
R1 in a 10k
D1 a 0 DM

.model DM D(IS=1e-14 CJO=2p M=0.33)
.tran 0.5n 150n
.end
