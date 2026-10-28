High-side gate drive: floating 10 V gate supply on a 48 V bus puts the gate at 58 V
* bench.id: rob_cv_clamp_highside_gate_58v
* bench.group: robustness
* bench.title: High-side gate drive: floating 10 V gate supply on a 48 V bus puts the gate at 58 V
* bench.probes: v(g), v(out), i(vbus)
* bench.expect: match
* bench.tags: nonlinear, mos, floating_source, high_side, op
* bench.note: ngspice-47 v(out)=0.1651829 V, v(g)=58 V; my own Level-1 KCL check of the ngspice-tight solution: residual 1e-14 A. Two unrelated +/-300 V dummy sources make RustSpice converge (a +40 V one does not: the 58 V node is above it). Calibration evidence: an independent dense f64 Newton (source stepping, gmin->0) matches ngspice-tight to 6e-13 V with KCL residual 1e-18 A and 150 random starts reach only this root; a single +100 V dummy (clamp [0,100 V]) already makes RustSpice match ngspice to 3e-14 V; the source shows the DC clamp is [min(0,V), max(0,V)] over all voltage sources when no I/G source exists, here [0,48 V].
VBUS bus 0 48
VGS g bus 10
M1 out g 0 0 nm w=100u l=1u
RL out 0 100
RX bus out 1k
.model nm nmos(vto=2 kp=50u)
.op
.end
