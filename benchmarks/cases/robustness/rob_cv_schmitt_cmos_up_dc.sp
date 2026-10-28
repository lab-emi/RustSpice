CMOS Schmitt trigger, input swept up 0 -> 3.3 V (hysteresis path)
* bench.id: rob_cv_schmitt_cmos_up_dc
* bench.group: robustness
* bench.title: CMOS Schmitt trigger, input swept up 0 -> 3.3 V (hysteresis path)
* bench.probes: v(out)
* bench.tol.abs: 1e-7
* bench.expect: match
* bench.tags: nonlinear, bistable, hysteresis, dc
* bench.note: Regression anchor; RustSpice matches ngspice-tight to 1.9e-8 V (tol.abs 1e-7 because the logic-low rail at ~3e-9 V is set by the 1e-12 S leakage). Probe is v(out) only: the internal stack nodes n1/n2 float when their devices are off and sit at gmin-defined values that differ by 1e-4 V between the tools (placement of gmin), which is not the subject of this case.
Vdd vdd 0 3.3
M1 n1 in 0 0 nm w=4u l=0.5u
M2 out in n1 0 nm w=4u l=0.5u
M3 n1 out vdd 0 nm w=2u l=0.5u
M4 n2 in vdd vdd pm w=8u l=0.5u
M5 out in n2 vdd pm w=8u l=0.5u
M6 n2 out 0 vdd pm w=4u l=0.5u
.model nm nmos(level=1 vto=0.6 kp=100u lambda=0.02)
.model pm pmos(level=1 vto=-0.6 kp=40u lambda=0.02)
Vin in 0 0
.dc Vin 0 3.3 0.05
.end
