Diode with series RS=20 ohm behind a 100 ohm resistor from 5 V: OP
* bench.id: cap_dev_dio_rs_op
* bench.group: capability
* bench.title: Diode with series RS=20 ohm behind a 100 ohm resistor from 5 V: OP
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, op, series_resistance, model_param
* bench.note: Gap closed 2026-10-28: RustSpice now models diode series resistance RS (internal node); the evidence below records the reference and the behaviour before the fix. new deck: `.op` counterpart of cap_dev_dio_rs/cap_dev_dio_fwd_1a_rs with a physical operating point; hand solution of 5-I*100=Vd+I*20, I=IS*(exp(Vd/VT)-1)+gmin*Vd gives v(a)=1.456166 V, I=35.438 mA = tight ngspice to 2e-7 V; RustSpice equals the same solve with RS=0
V1 in 0 5
R1 in a 100
D1 a 0 DM
.model DM D(IS=1e-14 RS=20)
.op
.end
