Negative rail: diode returned to a -5 V node (V2 0 2 5)
* bench.id: rob_cv_clamp_neg_rail
* bench.group: robustness
* bench.title: Negative rail: diode returned to a -5 V node (V2 0 2 5)
* bench.probes: v(2), v(3), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, floating_source, negative_rail, op
* bench.note: ngspice-47 v(3)=-4.2872385 V (numpy oracle agrees to 1e-12). Widening the clamp with two unrelated +/-300 V sources (1k loads) makes RustSpice converge, which isolates the clamp as the cause (a +100 V dummy alone is NOT enough: the clamp is [0,100 V] and still excludes -5 V; the pair +-100 V is, and then RustSpice matches ngspice to 5e-14 V). Calibration evidence: independent dense f64 Newton matches ngspice-tight to 1.2e-11 V, unique root. Same circuit as acc_dc_clamp_negative_rail_diode, which also probes i(v2); reversing V2 ("V2 0 2 -1", node 2 = +1 V) converges in RustSpice.
V1 1 0 5
V2 0 2 5
R1 1 3 1k
D1 3 2 dm
.model dm D(is=1e-14 n=1)
.op
.end
