Voltage source with both terminals on the same node
* bench.id: rob_cv_sing_vsrc_self_short_op
* bench.group: robustness
* bench.title: Voltage source with both terminals on the same node
* bench.probes: v(1)
* bench.expect: loud_error
* bench.tags: ill_posed, shorted_source, ngspice_error_ok, op
* bench.note: ngspice-47: "Fatal error: instance v1 is a shorted VSRC" (names the instance); RustSpice: "Singular Jacobian matrix encountered at column/row 1". Both refuse; ngspice message is the better diagnostic.
V1 1 1 5
R1 1 0 1k
.op
.end
