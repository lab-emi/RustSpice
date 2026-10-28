Diode recombination current ISR=1e-12, NR=2
* bench.id: cap_dev_dio_isr_nr
* bench.group: capability
* bench.title: Diode recombination current ISR=1e-12, NR=2
* bench.probes: i(v1)
* bench.tol.abs: 1e-15
* bench.expect: match
* bench.tags: diode, dc, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models recombination current ISR/NR; the evidence below records the reference and the behaviour before the fix. evidence: RustSpice output is bit-identical with and without ISR/NR; adding ISR*(exp(V/(NR*VT))-1)*((1-V/VJ)^2+0.005)^(M/2) to the IS term reproduces tight ngspice to 1e-5 relative; RustSpice is 20% low at 0.3 V and 94% low at 0.05 V (per-point relative test, tol.abs 1e-15; the largest absolute error, 2.4e-6 A, sits at 0.8 V where the relative error is only 9e-6, the largest relative error is at 0.05 V); the form factor ((1-V/VJ)^2+0.005)^(M/2) is dialect-defined (black-box fit of ngspice with the default VJ=1, M=0.5): the plain ISR*(exp(V/(NR*VT))-1) law differs from ngspice by at most 7.5%, so the RustSpice error (94%) is 12x larger than that dialect spread
V1 a 0 0
D1 a 0 DM
.model DM D(IS=1e-14 ISR=1e-12 NR=2)
.dc V1 0 0.8 0.05
.end
