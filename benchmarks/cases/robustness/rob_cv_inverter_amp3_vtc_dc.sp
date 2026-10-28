Three cascaded CMOS inverters (gain ~1e5), input swept 1.4..1.8 V in 2 mV steps through the transition
* bench.id: rob_cv_inverter_amp3_vtc_dc
* bench.group: robustness
* bench.title: Three cascaded CMOS inverters (gain ~1e5), input swept 1.4..1.8 V in 2 mV steps through the transition
* bench.probes: v(out), v(a), v(b)
* bench.tol.abs: 1e-8
* bench.expect: match
* bench.tags: nonlinear, mos, high_gain, dc
* bench.note: Regression anchor: a sweep through an extremely steep region is handled; RustSpice matches ngspice-tight to 1.1e-9 V.
Vdd vdd 0 3.3
Vin in 0 0
Mn1 a in 0 0 nm w=2u l=0.5u
Mp1 a in vdd vdd pm w=4u l=0.5u
Mn2 b a 0 0 nm w=2u l=0.5u
Mp2 b a vdd vdd pm w=4u l=0.5u
Mn3 out b 0 0 nm w=2u l=0.5u
Mp3 out b vdd vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.7 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.7 kp=40u lambda=0.02)
.dc Vin 1.4 1.8 0.002
.end
