DC transfer of a diode + 1k rectifier swept -10..10 V
* bench.id: rob_cv_rect_dc_sweep
* bench.group: robustness
* bench.title: DC transfer of a diode + 1k rectifier swept -10..10 V
* bench.probes: v(out)
* bench.expect: match
* bench.oracle: dc_nonlinear
* bench.tags: nonlinear, rectifier, dc
* bench.note: Regression anchor: negative source range and 9.3 V output both handled; RustSpice matches ngspice-tight to 3e-9 V.
V1 in 0 0
D1 in out dm
R1 out 0 1k
.model dm D(is=1e-14 n=1)
.dc V1 -10 10 0.5
.end
