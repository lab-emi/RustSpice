Negative capacitor and negative inductor in transient
* bench.id: cap_el_cl_negative_values_tran
* bench.group: capability
* bench.title: Negative capacitor and negative inductor in transient
* bench.probes: v(oc), v(ol)
* bench.expect: match
* bench.tags: passive, tran, negative_value
* bench.note: Negative reactive elements give exponentially growing responses: tau = 667 ohm * -1 nF = -0.67 us for the RC branch and -1 uH / 1 ohm = -1 us for the RL branch, so both grow by about e^1.5 and e^1 over the 1 us run. ngspice accepts negative values. Closed forms (ideal step, ngspice agrees to 7e-7 and 3e-7 V): v(oc) = (2/3)*(1-exp(t/0.667us)) reaches -2.319 V at 1 us, v(ol) = exp(t/1us) reaches 2.717 V. The tran_lti oracle does not accept negative values, so the evidence is this closed form; RustSpice differs from the reference by 1.1e-3 / 5.6e-4 of the swing (fixed-step error on growing exponentials, below the 5e-3 default).
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in oc 1k
R2 oc 0 2k
C1 oc 0 -1n
V2 in2 0 PULSE(0 1 0 1n 1n 1 2)
R3 in2 ol 1
L1 ol 0 -1u
.tran 10n 1u
.end
