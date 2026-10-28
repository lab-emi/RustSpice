Diode IS=1e-16 from 5 V through 1 kohm: OP (control, RustSpice converges correctly)
* bench.id: cap_dev_dio_is_small_op_5v_ctl
* bench.group: capability
* bench.title: Diode IS=1e-16 from 5 V through 1 kohm: OP (control, RustSpice converges correctly)
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, op, small_is, control
* bench.note: new deck: control for cap_dev_dio_is_small_op_2v (same IS=1e-16, same `.op`): here the Newton path from 0 V reaches the true solution v(a)=0.81128 V, so the false-convergence defect depends on the circuit, not just on IS; regression anchor, non-vacuous (v(a)=0.8113 V, i=4.19 mA; the diode is conducting)
V1 in 0 5
R1 in a 1k
D1 a 0 DM
.model DM D(IS=1e-16)
.op
.end
