LED-like diode IS=1e-20 N=2 from 5 V through 330 ohm
* bench.id: acc_dc_diode_fc_led_is1em20_n2
* bench.group: accuracy
* bench.title: LED-like diode IS=1e-20 N=2 from 5 V through 330 ohm
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.expect: match
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, small_is, silent_risk, led
* bench.note: oracle and tight ngspice agree to 6e-14 relative
V1 in 0 5
R1 in d 330
D1 d 0 dm
.model dm D(IS=1e-20 N=2)
.op
.end
