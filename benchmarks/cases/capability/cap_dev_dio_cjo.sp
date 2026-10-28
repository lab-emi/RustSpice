Diode junction capacitance CJO=2 pF (reverse-bias RC charge)
* bench.id: cap_dev_dio_cjo
* bench.group: capability
* bench.title: Diode junction capacitance CJO=2 pF (reverse-bias RC charge)
* bench.probes: v(a)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: CJO used as a constant capacitance, VJ/M ignored (identical waveform for CJO, CJO+VJ, CJO+M): norm err 0.15
* bench.tags: diode, tran, capacitance, model_param
* bench.note: evidence: RustSpice treats CJO as a constant capacitance (2.7 V away from the CJO-free waveform) but ignores the default VJ=1/M=0.5 grading; own ODE solution (python Radau, C=CJO/sqrt(1-V/VJ)) equals the converged ngspice reference to 4e-7 normalised (ngspice as-is 3e-4), RustSpice 0.15
V1 in 0 PULSE(0 -3 5n 5n 5n 400n 800n)
R1 in a 10k
D1 a 0 DM

.model DM D(IS=1e-14 CJO=2p)
.tran 0.5n 150n
.end
