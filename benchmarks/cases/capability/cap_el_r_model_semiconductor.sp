Semiconductor resistor model (rsh, w, l)
* bench.id: cap_el_r_model_semiconductor
* bench.group: capability
* bench.title: Semiconductor resistor model (rsh, w, l)
* bench.probes: i(v1)
* bench.expect: match
* bench.tags: passive, op, model, unsupported
* bench.note: Gap closed 2026-10-28: RustSpice now models .model R (semiconductor resistor); the evidence below records the reference and the behaviour before the fix. Closed form R = rsh*l/w = 100*100u/12u = 833.33 ohm, i(v1) = -1.2 mA; ngspice-47 returns -1.2000 mA. narrow is deliberately 0 so the closed form is convention-free: with narrow=1u ngspice-47 returns 1 kohm (w reduced by 2*narrow), a convention-specific detail that another implementation could handle differently and fail_accuracy instead of closing the gap. Loud refusal of a supported feature (R model card) is expect=match + unsupported-device-loud (SUITE_SPEC 13.1).
V1 a 0 1
.model rmod R(rsh=100 narrow=0)
R1 a 0 rmod w=12u l=100u
.op
.end
