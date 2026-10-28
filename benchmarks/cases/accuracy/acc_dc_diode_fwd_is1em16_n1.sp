Diode IS=1e-16 N=1 from 5 V through 1k: just above the small-IS failure region (control)
* bench.id: acc_dc_diode_fwd_is1em16_n1
* bench.group: accuracy
* bench.title: Diode IS=1e-16 N=1 from 5 V through 1k: just above the small-IS failure region (control)
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-12
* bench.oracle: dc_nonlinear(gmin=1e-12)
* bench.tags: nonlinear, op, diode, small_is, control
* bench.note: tol.abs tightened to 1e-12 A (the default 1e-9 A would be 1e-5 relative on mA currents)
V1 in 0 5
R1 in d 1k
D1 d 0 dm
.model dm D(IS=1e-16 N=1)
.op
.end
