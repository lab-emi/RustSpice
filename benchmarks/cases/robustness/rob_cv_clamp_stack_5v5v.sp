Series-stacked supplies (V2 sits on V1): node 2 at 10 V feeding a diode load
* bench.id: rob_cv_clamp_stack_5v5v
* bench.group: robustness
* bench.title: Series-stacked supplies (V2 sits on V1): node 2 at 10 V feeding a diode load
* bench.probes: v(2), v(3), i(v1)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, floating_source, stacked_rail, op
* bench.note: ngspice-47 solves (v(3)=0.7127615 V); my independent numpy Newton agrees to 1e-12. Adding an unrelated 40 V source with a 1k load (widens the clamp to [0,40 V]) makes RustSpice converge, which isolates the clamp as the cause (with a +100 V dummy RustSpice matches ngspice to 5e-14 V). Calibration evidence: independent dense f64 Newton matches ngspice-tight to 2e-12 V (residual 2e-17 A), unique root. Same circuit as acc_dc_clamp_stacked_vsrc_diode, which also probes i(v2).
V1 1 0 5
V2 2 1 5
R1 2 3 1k
D1 3 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
