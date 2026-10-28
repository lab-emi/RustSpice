PWL sources at .op take their t=0 value (voltage and current source)
* bench.id: cap_el_i_pwl_op_t0
* bench.group: capability
* bench.title: PWL sources at .op take their t=0 value (voltage and current source)
* bench.probes: v(a), v(b)
* bench.tol.rel: 1e-9
* bench.tol.abs: 1e-12
* bench.floor: 1e-6
* bench.expect: match
* bench.tags: source, op, waveform, pwl, current_source, silent_risk
* bench.note: ngspice-47 .op gives I1 = 3 mA (the first PWL value) so v(a)=3 V, and V1 = 0.7 V (the first PWL value). Verified by hand against the PWL definition.
I1 0 a PWL(0 3m 1u 0 2u 5m)
R1 a 0 1k
V1 b 0 PWL(0 0.7 1u 3)
R2 b 0 1k
.op
.end
