MOS bias: 100 uA into 100k drives the gate node to 10 V (supply is 3 V)
* bench.id: rob_cv_clamp_isrc_mos_gate
* bench.group: robustness
* bench.title: MOS bias: 100 uA into 100k drives the gate node to 10 V (supply is 3 V)
* bench.probes: v(g), v(d), i(vdd)
* bench.expect: match
* bench.tags: nonlinear, mos, current_driven, op
* bench.note: ngspice-47 v(d)=0.0319693 V, v(g)=10 V. My own textbook Level-1 evaluator confirms the ngspice-tight solution satisfies KCL to 1e-14 A. An unrelated +40 V dummy source (1k load) makes RustSpice converge. Calibration evidence: independent dense f64 Newton (gmin->0) matches ngspice-tight to 4e-12 V (residual 1e-19 A), unique root; with a +100 V dummy RustSpice matches ngspice to 8e-13 V.
I1 0 g 100u
R1 g 0 100k
M1 d g 0 0 nm w=10u l=1u
VDD vdd 0 3
RD vdd d 10k
.model nm nmos(level=1 vto=0.7 kp=100u)
.op
.end
