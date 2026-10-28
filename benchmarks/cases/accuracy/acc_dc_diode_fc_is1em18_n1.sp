Diode IS=1e-18 N=1 from 5 V through 1k
* bench.id: acc_dc_diode_fc_is1em18_n1
* bench.group: accuracy
* bench.title: Diode IS=1e-18 N=1 from 5 V through 1k
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, small_is, silent_risk
* bench.note: oracle and tight ngspice agree to 6e-14 relative. Same circuit and values as cap_dev_dio_is_small_op (duplicate witness: it is counted twice in the gap tally).
V1 in 0 5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-18 N=1)
.op
.end
