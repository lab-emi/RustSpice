LED-like diode IS=1e-18 N=1.5, 5 V through 330 ohm
* bench.id: rob_cv_diode_led_is1e18_n15_op
* bench.group: robustness
* bench.title: LED-like diode IS=1e-18 N=1.5, 5 V through 330 ohm
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, silent_risk, op
* bench.note: ngspice-47 gives v(2)=1.4323735 V (current 10.8 mA); independent numpy Newton agrees. Same defect for IS=1e-20 N=2 (RustSpice 4.99999999 vs 2.1366796) and IS=1e-21 (4.999999995 vs 1.1071651). Mechanism check: the Python re-implementation of the documented Newton loop reproduces the false convergence in 2 iterations (v(2)=4.9999999967 V) and, with the limiter state replaced by the raw junction voltage, converges to the correct 1.4323734803 V (99 iterations).
V1 1 0 5
R1 1 2 330
D1 2 0 led
.model led D(is=1e-18 n=1.5)
.op
.end
