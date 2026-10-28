Diode IS=1e-14 N=0.1 (2.6 mV per e-fold) from 5 V through 1 ohm: very steep exponential
* bench.id: acc_dc_diode_fwd_steep_n0p1
* bench.group: accuracy
* bench.title: Diode IS=1e-14 N=0.1 (2.6 mV per e-fold) from 5 V through 1 ohm: very steep exponential
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, steep_exponential
* bench.note: tol.abs tightened to 1e-12 A (the default 1e-9 A would be 1e-5 relative on mA currents); v(d) = 0.0875 V, i = 4.91 A
V1 in 0 5
R1 in d 1
D1 d 0 dm
.model dm D(IS=1e-14 N=0.1)
.op
.end
