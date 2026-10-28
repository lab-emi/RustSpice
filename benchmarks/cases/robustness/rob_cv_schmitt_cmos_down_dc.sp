CMOS Schmitt trigger, input swept down 3.3 -> 0 V (opposite hysteresis branch)
* bench.id: rob_cv_schmitt_cmos_down_dc
* bench.group: robustness
* bench.title: CMOS Schmitt trigger, input swept down 3.3 -> 0 V (opposite hysteresis branch)
* bench.probes: v(out)
* bench.tol.abs: 1e-7
* bench.expect: match
* bench.tags: nonlinear, bistable, hysteresis, dc
* bench.note: Regression anchor for the other branch of the hysteresis loop; RustSpice matches ngspice-tight to 6.4e-9 V; probe is v(out) only (see the up-sweep deck).
Vdd vdd 0 3.3
M1 n1 in 0 0 nm w=4u l=0.5u
M2 out in n1 0 nm w=4u l=0.5u
M3 n1 out vdd 0 nm w=2u l=0.5u
M4 n2 in vdd vdd pm w=8u l=0.5u
M5 out in n2 vdd pm w=8u l=0.5u
M6 n2 out 0 vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.6 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.6 kp=40u lambda=0.02)
Vin in 0 3.3
.dc Vin 3.3 0 -0.05
.end
