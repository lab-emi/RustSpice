Diode IS=1e-16, 1 V source through 100 ohm
* bench.id: rob_cv_diode_is1e16_1v_op
* bench.group: robustness
* bench.title: Diode IS=1e-16, 1 V source through 100 ohm
* bench.probes: v(2), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, diode, silent_risk, op
* bench.note: ngspice-47 (as-is and tight) gives v(2)=0.7930429 V; the independent numpy Newton agrees (diode current 2.07 mA = (1-0.793)/100). RustSpice returns exit code 0 with a diode carrying 0 A: confident wrong numbers. The same IS=1e-16 diode at 5 V / 1k converges correctly, and IS=1e-14 at 5 V / 1k too: the defect depends on IS and drive together. Mechanism check: a Python re-implementation of the documented Newton loop (pnjlim state kept in diode_vd, convergence test on the linearised residual and on the step of x) reproduces RustSpice's false convergence in 2 iterations at v(2)=0.9999999998 V, and with the limiter state replaced by the raw junction voltage converges in 14 iterations to the correct 0.7930428677 V: the criterion never checks that the limited junction voltage equals the actual one (SPICE's NOCON flag).
V1 1 0 1
R1 1 2 100
D1 2 0 dm
.model dm D(is=1e-16 n=1)
.op
.end
