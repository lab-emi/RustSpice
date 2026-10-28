1 fA into a diode (below RustSpice abstol 1e-12 A; answer 0.7184 mV)
* bench.id: rob_cv_scale_1fa_diode_op
* bench.group: robustness
* bench.title: 1 fA into a diode (below RustSpice abstol 1e-12 A; answer 0.7184 mV)
* bench.probes: v(1)
* bench.tol.rel: 1e-6
* bench.tol.abs: 1e-11
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, tiny_value, op
* bench.note: Regression anchor: although the KCL residual criterion (1e-12 A) is 1000x the source current, the step criterion still drives RustSpice to the correct 0.7183685 mV (|d|=1.4e-12 V).
I1 0 1 1f
D1 1 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
