Diode IS=1e-9 reverse biased by 100 mV through 100 ohm: transition of the SPICE3 reverse characteristic
* bench.id: acc_dc_diode_rev_is1em9_n1_100mv
* bench.group: accuracy
* bench.title: Diode IS=1e-9 reverse biased by 100 mV through 100 ohm: transition of the SPICE3 reverse characteristic
* bench.probes: v(d), i(v1)
* bench.tol.abs: 1e-18
* bench.floor: 1e-14
* bench.tags: nonlinear, op, diode, reverse
* bench.note: no dc_nonlinear oracle: below -3 n vt the SPICE3 diode follows a cubic reverse law that differs from ideal Shockley by 2e-3 of IS (ngspice and RS agree with each other to 1e-10); tol.abs lowered to 1e-18 A
V1 in 0 -0.1
R1 in d 100
D1 d 0 dm
.model dm D(IS=1e-9 N=1)
.op
.end
