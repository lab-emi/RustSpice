Diode straight across 1 kV (exp overflow: argument 38 000)
* bench.id: rob_cv_diode_overflow_1kv_op
* bench.group: robustness
* bench.title: Diode straight across 1 kV (exp overflow: argument 38 000)
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: nonlinear, diode, overflow, ngspice_error_ok, op
* bench.note: ngspice-47 aborts too (exit code 1, "DC solution failed" after dynamic and true gmin stepping, no output); RustSpice stops after 150 Newton iterations with a finite residual message (4e21), no panic and no numbers. Both refuse: this pins RustSpice to refusing rather than emitting inf/NaN values.
V1 1 0 1000
D1 1 0 dm
.model dm D(is=1e-14 n=1)
.op
.end
