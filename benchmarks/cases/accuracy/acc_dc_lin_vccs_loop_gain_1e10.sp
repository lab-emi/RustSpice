Non-inverting amplifier built from a VCCS stage: open-loop gain 1e12, closed-loop gain 10
* bench.id: acc_dc_lin_vccs_loop_gain_1e10
* bench.group: accuracy
* bench.title: Non-inverting amplifier built from a VCCS stage: open-loop gain 1e12, closed-loop gain 10
* bench.probes: v(o), v(fb)
* bench.tol.rel: 1e-8
* bench.tol.abs: 1e-12
* bench.oracle: dc_linear
* bench.tags: linear, op, vccs, feedback, ill_conditioned, high_gain
* bench.note: loop gain 1e11 (matrix condition ~1e12); RS and ngspice both reach 2e-11 relative; tol.rel set to 1e-8
Vin in 0 1
G1 0 o in fb 1k
R1 o 0 1g
Rf o fb 9k
Rg fb 0 1k
.op
.end
