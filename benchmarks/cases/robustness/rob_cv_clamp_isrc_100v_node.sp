Current-driven node at 100 V (1 mA into 100k) next to a current-driven diode
* bench.id: rob_cv_clamp_isrc_100v_node
* bench.group: robustness
* bench.title: Current-driven node at 100 V (1 mA into 100k) next to a current-driven diode
* bench.probes: v(1), v(3)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, current_driven, op
* bench.note: ngspice-47 v(1)=100 V, v(3)=0.6551179 V (numpy oracle agrees). Two decoupled sub-nets: the diode only makes the circuit nonlinear (a purely linear deck takes the linear fast path and passes). Calibration evidence: independent dense f64 Newton (gmin->0) matches ngspice-tight to 1.7e-11 V (v(1)=100 V exactly, v(3)=0.6551179 V), unique root; adding one unrelated +100 V source with a 1k load (clamp [-100,200 V]) makes RustSpice match ngspice to 4e-14 V.
I1 0 1 1m
R1 1 0 100k
I2 0 3 1m
D1 3 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
