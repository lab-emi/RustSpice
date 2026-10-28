Diode IS=1e-18 from 5 V through 1 kohm: OP
* bench.id: cap_dev_dio_is_small_op
* bench.group: capability
* bench.title: Diode IS=1e-18 from 5 V through 1 kohm: OP
* bench.probes: v(a), i(v1)
* bench.expect: match
* bench.tags: diode, op, small_is, silent_risk
* bench.note: evidence: hand-solved Shockley (mpmath 40 digits, gmin 1e-12): v(a)=0.92965 V, i=4.0703 mA; tight ngspice agrees to 3e-7 V (that gap is only the kT/q constant: with kT/q = 0.0258649170071575 V (the constant behind ngspice-47) the mpmath value 0.9296499193 V equals tight ngspice to 3e-17 relative) and RustSpice's own `.dc` control deck reaches 0.929650 V at 5 V, but RustSpice `.op` stops after 2 Newton iterations at v(a)=4.999999995 V (diode off, 5e-12 A, residual taken on the limited junction voltage); python scan of 135 (IS,V,R) combinations vs mpmath: wrong in 56, all with IS<=1e-16 (see cap_dev_dio_is_small_op_2v), none of the 45 with IS>=1e-15. Same circuit and values as acc_dc_diode_fc_is1em18_n1 (duplicate witness: it is counted twice in the gap tally).
V1 in 0 5
R1 in a 1k
D1 a 0 DM
.model DM D(IS=1e-18)
.op
.end
