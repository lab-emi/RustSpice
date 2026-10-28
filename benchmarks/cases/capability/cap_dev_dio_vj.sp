Diode junction potential VJ=0.4 V with CJO=2 pF
* bench.id: cap_dev_dio_vj
* bench.group: capability
* bench.title: Diode junction potential VJ=0.4 V with CJO=2 pF
* bench.probes: v(a)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: VJ ignored: waveform identical to the CJO-only deck (constant capacitance): norm err 0.26
* bench.tags: diode, tran, capacitance, model_param
* bench.note: evidence: RustSpice output is bit-identical with and without VJ=0.4; own ODE solution (python Radau, graded junction capacitance) equals the converged ngspice reference to 4.8e-7 normalised (ngspice as-is 3e-4), RustSpice 0.26
V1 in 0 PULSE(0 -3 5n 5n 5n 400n 800n)
R1 in a 10k
D1 a 0 DM

.model DM D(IS=1e-14 CJO=2p VJ=0.4)
.tran 0.5n 150n
.end
