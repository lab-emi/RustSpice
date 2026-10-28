Diode IS=1e-24 N=2 from 12 V through 100 ohm: extreme small IS under hard drive
* bench.id: acc_dc_diode_fc_is1em24_n2_12v
* bench.group: accuracy
* bench.title: Diode IS=1e-24 N=2 from 12 V through 100 ohm: extreme small IS under hard drive
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, small_is, silent_risk
* bench.note: oracle and tight ngspice agree to 6e-14 relative
V1 in 0 12
R1 in d 100
D1 d 0 dm
.model dm D(IS=1e-24 N=2)
.op
.end
