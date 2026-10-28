Diode with series RS=20 ohm behind a 100 ohm resistor from 5 V: OP
* bench.id: cap_dev_dio_rs_op
* bench.group: capability
* bench.title: Diode with series RS=20 ohm behind a 100 ohm resistor from 5 V: OP
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.known_gap: silent-ignore-model-param
* bench.gap.note: RS ignored: v(a)=0.7521 V and 42.5 mA (ngspice 1.4562 V and 35.4 mA); the failure is moderate, not an exponent overflow, so it is silent in a realistic circuit
* bench.tags: diode, op, series_resistance, model_param
* bench.note: new deck: `.op` counterpart of cap_dev_dio_rs/cap_dev_dio_fwd_1a_rs with a physical operating point; hand solution of 5-I*100=Vd+I*20, I=IS*(exp(Vd/VT)-1)+gmin*Vd gives v(a)=1.456166 V, I=35.438 mA = tight ngspice to 2e-7 V; RustSpice equals the same solve with RS=0
V1 in 0 5
R1 in a 100
D1 a 0 DM
.model DM D(IS=1e-14 RS=20)
.op
.end
