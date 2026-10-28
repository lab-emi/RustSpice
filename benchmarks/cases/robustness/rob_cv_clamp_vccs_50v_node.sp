VCCS gain stage (gm=5 mS into 10k) puts a node at 50 V, plus a diode branch
* bench.id: rob_cv_clamp_vccs_50v_node
* bench.group: robustness
* bench.title: VCCS gain stage (gm=5 mS into 10k) puts a node at 50 V, plus a diode branch
* bench.probes: v(2), v(3)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, controlled_source, op
* bench.note: ngspice-47 v(2)=50 V, v(3)=0.6294407 V (numpy oracle agrees). An unrelated +40 V dummy source (1k load) makes RustSpice converge. Calibration evidence: independent dense f64 Newton (gmin->0) matches ngspice-tight to 4e-11 V (v(2)=50 V, residual 4e-19 A), unique root; with a +100 V dummy RustSpice matches ngspice to 7e-12 V.
Vin 1 0 1
G1 0 2 1 0 5m
R1 2 0 10k
R3 1 3 1k
D1 3 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
