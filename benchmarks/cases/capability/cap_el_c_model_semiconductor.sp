Semiconductor capacitor model (cj, w, l)
* bench.id: cap_el_c_model_semiconductor
* bench.group: capability
* bench.title: Semiconductor capacitor model (cj, w, l)
* bench.probes: v(a)
* bench.expect: match
* bench.known_gap: unsupported-device-loud
* bench.gap.note: the card is refused at parse time (`Invalid capacitance value cmod`, exit 1, no panic): a model name instead of a value is not understood, where ngspice computes C = 1 uF
* bench.tags: passive, tran, model, unsupported
* bench.note: Closed form: C = cj*w*l = 1e-3 F/m^2 * 1 m * 1 mm = 1 uF, tau = 1k*1u = 1 ms, v(a) = 1-exp(-t/1ms); ngspice-47 (reltol 1e-6, tmax 1u) agrees with the closed form to 5e-7 over the 5 ms. Loud refusal of a supported feature (C model card) is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 in 0 PULSE(0 1 0 1n 1n 1 2)
R1 in a 1k
.model cmod C(cj=1e-3 narrow=0)
C1 a 0 cmod w=1 l=1m
.tran 20u 5m
.end
